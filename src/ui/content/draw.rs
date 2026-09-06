//! Main content-pane renderer.
//!
//! `draw_content` renders the right-hand panel across its four modes: a styled
//! unified/side-by-side diff (no gutter, no selection), a plugin-rendered view
//! from precomputed spans, a memory-mapped virtual-file view that highlights
//! only the visible window on the fly, and an inline fallback for errors,
//! binaries, and small buffers. It draws the line-number and fold gutters,
//! applies word wrap, and layers in-file search and text-selection highlighting
//! plus the transient scrollbar by calling the sibling helpers. It also records
//! the content `Rect` and scroll offsets back onto `App` for mouse hit-testing.

use ratatui::{
    layout::{Alignment, Constraint, Direction, Layout, Rect},
    style::Style,
    text::{Line, Span},
    widgets::{Block, Borders, Clear, Paragraph, Wrap},
    Frame,
};
use unicode_width::UnicodeWidthStr;

use crate::app::{App, Focus};
use crate::search::InFileSearch;

use super::blame;
use super::diff::draw_side_by_side_diff;
use super::draw_text::{render_inline_fallback, render_virtual_file, wrap_content};
use super::scrollbar::draw_content_scrollbar;
use super::search::apply_search_to_regions;
use super::selection::apply_selection;

/// Renders the content/diff panel. Handles four modes:
/// - Diff view (styled per-line, no gutter, no selection)
/// - Plugin-rendered view (styled spans)
/// - Virtual file view (mmap-backed, syntax-highlighted on the fly for the visible window)
/// - Inline fallback view (for errors, binaries, small files)
pub(crate) fn draw_content(f: &mut Frame, app: &mut App, area: Rect) {
    let focused = matches!(app.focus, Focus::Content)
        && app.search.is_none()
        && app.history.is_none()
        && app.theme_picker.is_none();
    let border_style = if focused {
        Style::default().fg(app.theme.accent)
    } else {
        Style::default().fg(app.theme.dim)
    };

    let mut title = if let Some(t) = &app.content_title {
        t.clone()
    } else {
        app.current_file
            .as_ref()
            .and_then(|p| p.strip_prefix(&app.root).ok())
            .map(|rel| format!(" {} ", rel.display()))
            .unwrap_or_else(|| " No file ".into())
    };
    // While a background load is in flight the previous file's content stays on
    // screen; flag it so fast loads are invisible and slow ones are explained.
    if app.loading {
        title.push_str(" loading… ");
    }

    let block = Block::default()
        .title(title)
        .borders(Borders::ALL)
        .border_style(border_style);

    let mut inner = block.inner(area);
    let mut line_blame_area = Rect::default();

    // Cleared here; the image branch below is the only path that sets a real
    // rect, so any other view leaves the post-frame overlay with nothing to do.
    app.image_area = Rect::default();

    // Side-by-side diff layout takes over the whole content pane when toggled
    // on and the pane is wide enough; otherwise we fall through to unified.
    if app.is_diff
        && app.diff_side_by_side
        && !app.diff_rows.is_empty()
        && inner.width >= crate::diff::MIN_SIDE_BY_SIDE_WIDTH
    {
        draw_side_by_side_diff(f, app, area, block);
        return;
    }

    // Inline image preview: an image file on a Kitty-graphics terminal. The
    // bitmap itself is written straight to the terminal by
    // `graphics::render_overlay` after ratatui paints this frame; here we just
    // paint the border, blank the region it will occupy, and caption it.
    if app.content_image.is_some() && !app.is_diff {
        draw_image_preview(f, app, area, block);
        return;
    }

    // ── Line blame bar: reserve 2 rows at the bottom ───────────────
    if app.show_line_blame && app.has_text_cursor() && app.current_file.is_some() {
        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([Constraint::Min(0), Constraint::Length(2)])
            .split(inner);
        inner = chunks[0];
        line_blame_area = chunks[1];
    }

    // ── Full-file blame: split inner horizontally ───────────────────
    let mut blame_area = Rect::default();
    if app.show_blame && app.has_text_cursor() {
        if let Some(ref path) = app.current_file.clone() {
            let bw = blame::blame_strip_width(inner.width);
            if bw < inner.width {
                let chunks = Layout::default()
                    .direction(Direction::Horizontal)
                    .constraints([Constraint::Length(bw), Constraint::Min(0)])
                    .split(inner);
                let blame_lines = crate::git::file_blame(&app.root, path);
                if !blame_lines.is_empty() {
                    blame::draw_blame_annotations(f, app, chunks[0], &blame_lines);
                    blame_area = chunks[0];
                    inner = chunks[1];
                }
            }
        }
    }

    let view_height = inner.height as usize;
    let total_lines = app.display_line_count();
    let scroll = app.content_scroll.min(app.content_scroll_max());
    let (render_scroll, leading_rows) = if app.word_wrap {
        app.wrapped_display_position(scroll)
    } else {
        (scroll, 0)
    };
    let visible_end = if app.word_wrap {
        let mut end = render_scroll;
        while end < total_lines
            && app.wrapped_line_start(end + 1).saturating_sub(scroll) < view_height
        {
            end += 1;
        }
        end.saturating_add(1).min(total_lines)
    } else {
        (render_scroll + view_height).min(total_lines)
    };

    // Rendered-content source from plugins.
    let render_lines: Option<&Vec<Vec<(ratatui::style::Style, String)>>> = app
        .current_file
        .as_ref()
        .and_then(|p| app.plugin_content.get(p));

    let sel = app.selection.as_ref().map(|s| s.normalized());
    let sel_bg = app.theme.selection_bg;
    let in_file_search = app.in_file_search.as_ref();

    let show_ln = app.show_line_numbers;

    // ln_width, ln_lines, content_lines, fold_gutter_rows
    let (ln_width, mut ln_lines, mut content_lines, mut new_fold_gutter_rows) = if app.is_diff {
        // Diff view: iterate all highlighted lines (diffs are never large).
        let lines = app
            .highlighted
            .iter()
            .enumerate()
            .map(|(i, spans)| {
                let regions_owned: Vec<(Style, String)> =
                    spans.iter().map(|(s, t)| (*s, t.clone())).collect();
                Line::from(if let Some(s) = in_file_search {
                    apply_search_to_regions(&regions_owned, i, s, &app.theme)
                } else {
                    regions_owned
                        .iter()
                        .map(|(s, t)| Span::styled(t.clone(), *s))
                        .collect()
                })
            })
            .collect();
        (0, vec![], lines, vec![])
    } else if app.is_json && app.show_pretty_json && !app.json_pretty_lines.is_empty() {
        render_preformatted_lines(
            app,
            &app.json_pretty_lines,
            render_scroll,
            visible_end,
            show_ln,
            in_file_search,
            sel,
            sel_bg,
        )
    } else if app.is_csv && app.show_csv_table && !app.csv_table_lines.is_empty() {
        render_preformatted_lines(
            app,
            &app.csv_table_lines,
            render_scroll,
            visible_end,
            show_ln,
            in_file_search,
            sel,
            sel_bg,
        )
    } else if let Some(md_lines) = render_lines {
        // Rendered content (plugin): iterate only the visible
        // window of pre-rendered lines. Line numbers are hidden for rendered
        // content since rendered-line indices don't correspond to source lines
        // (rendering collapses blank lines, strips code fences, etc.). This
        // matches line_prefix_width() which already returns 0 for rendered
        // markdown, keeping input and render math consistent.
        let lines: Vec<Line> = md_lines[render_scroll..visible_end]
            .iter()
            .enumerate()
            .map(|(offset, spans)| {
                let logical_idx = render_scroll + offset;
                let regions_owned: Vec<(Style, String)> =
                    spans.iter().map(|(s, t)| (*s, t.clone())).collect();
                if let Some(s) = in_file_search {
                    Line::from(apply_search_to_regions(
                        &regions_owned,
                        logical_idx,
                        s,
                        &app.theme,
                    ))
                } else if let Some(((sl, sc), (el, ec))) = sel {
                    if logical_idx >= sl && logical_idx <= el {
                        let col_start = if logical_idx == sl { sc } else { 0 };
                        let col_end = if logical_idx == el { ec } else { usize::MAX };
                        Line::from(apply_selection(
                            &regions_owned,
                            col_start,
                            col_end,
                            sel_bg,
                            app.theme.is_monochrome(),
                        ))
                    } else {
                        Line::from(
                            regions_owned
                                .iter()
                                .map(|(s, t)| Span::styled(t.clone(), *s))
                                .collect::<Vec<_>>(),
                        )
                    }
                } else {
                    Line::from(
                        regions_owned
                            .iter()
                            .map(|(s, t)| Span::styled(t.clone(), *s))
                            .collect::<Vec<_>>(),
                    )
                }
            })
            .collect();
        (0, vec![], lines, vec![])
    } else if let Some(vf) = app.virtual_file.as_ref() {
        render_virtual_file(
            app,
            vf,
            inner,
            render_scroll,
            visible_end,
            show_ln,
            in_file_search,
            sel,
            sel_bg,
        )
    } else {
        render_inline_fallback(
            app,
            inner,
            render_scroll,
            visible_end,
            show_ln,
            in_file_search,
            sel,
            sel_bg,
        )
    };

    // Empty state hint for first-time users: show orientation if no file open.
    if content_lines.is_empty() && app.current_file.is_none() {
        let search_key = app.keys().label_for_action("search_files");
        let open_key = app.keys().label_for_action("tree_expand");
        let help_key = app.keys().label_for_action("help");
        if !search_key.is_empty() && !open_key.is_empty() {
            let help_hint = if help_key.is_empty() {
                String::new()
            } else {
                format!(" · {help_key} for help")
            };
            content_lines.push(Line::from(Span::styled(
                format!(" Press {search_key} to search, or {open_key} to open a file{help_hint}"),
                Style::default().fg(app.theme.dim),
            )));
            // Keep ln_lines in sync with content_lines so wrap_content's zip
            // (which stops at the shorter side) doesn't drop this line when
            // word wrap is on and the line-number gutter is showing.
            if ln_width > 0 {
                ln_lines.push(Line::from(""));
            }
        }
    }

    // Word-wrap expansion: break content + gutters into visual rows so they
    // stay aligned under wrap (ratatui's Wrap can't communicate row count to
    // the gutter Paragraph, causing cumulative drift on each wrapped line).
    let mut visual_to_display: Vec<usize> = if app.word_wrap && ln_width > 0 {
        let cw = inner.width.saturating_sub(ln_width as u16) as usize;
        if cw > 0 {
            let (exp_gutters, exp_content, vmap, updated_fold) = wrap_content(
                &content_lines,
                &ln_lines,
                cw,
                inner.y,
                &new_fold_gutter_rows,
            );
            ln_lines = exp_gutters;
            content_lines = exp_content;
            new_fold_gutter_rows = updated_fold;
            vmap
        } else {
            (0..content_lines.len()).collect()
        }
    } else {
        (0..content_lines.len()).collect()
    };

    if app.word_wrap && ln_width > 0 && leading_rows > 0 {
        let skip = leading_rows.min(content_lines.len());
        content_lines.drain(..skip);
        ln_lines.drain(..skip);
        // The map is parallel to the expanded rows and must be clipped with
        // them, otherwise active-line highlighting is shifted after the first
        // partially visible wrapped line.
        visual_to_display.drain(..skip);
    }

    // Active-line highlight: full-width row background + gutter caret.
    if app.has_text_cursor() && !app.diff_sbs_active() {
        let active_bg = app.theme.active_line_bg;
        let content_w = inner.width.saturating_sub(ln_width as u16) as usize;
        // Selection takes visual precedence: when it covers the active line
        // (always true mid-drag, since the drag anchor sets the cursor), skip
        // the row background so the selection_bg stays visible. The gutter
        // caret below still paints - the gutter is never part of a selection.
        let sel_covers_active = sel.is_some_and(|(start, end)| {
            let active_physical = app.display_to_physical(app.active_line);
            // apply_selection highlights a half-open [col_start, col_end) range,
            // so an end column of 0 leaves the end line unhighlighted; exclude
            // it here too or the active-line background would be suppressed
            // on a line that has no visible selection.
            let last_covered = if end.1 == 0 {
                end.0.saturating_sub(1)
            } else {
                end.0
            };
            start != end && (start.0..=last_covered).contains(&active_physical)
        });
        for (j, line) in content_lines.iter_mut().enumerate() {
            let display_line = visual_to_display.get(j).copied().unwrap_or(0);
            if sel_covers_active || render_scroll + display_line != app.active_line {
                continue;
            }
            // Full-width content highlight
            for span in &mut line.spans {
                span.style = span.style.bg(active_bg);
            }
            let text_w: usize = line.spans.iter().map(|s| s.content.as_ref().width()).sum();
            if text_w < content_w {
                line.spans.push(Span::styled(
                    " ".repeat(content_w - text_w),
                    Style::default().bg(active_bg),
                ));
            }
        }
        // Gutter caret: brighten the active line's gutter foreground
        for (j, gutter) in ln_lines.iter_mut().enumerate() {
            let display_line = visual_to_display.get(j).copied().unwrap_or(0);
            if render_scroll + display_line == app.active_line {
                for span in &mut gutter.spans {
                    span.style = span.style.bg(active_bg).fg(app.theme.accent);
                }
            }
        }
    }

    f.render_widget(block, area);

    let hscroll = if app.word_wrap {
        0
    } else {
        app.content_hscroll as u16
    };

    // Fixed gutter: line numbers are pre-clipped to the visible range,
    // rendered with scroll=(0,0) because they are already at the right offset.
    if ln_width > 0 {
        f.render_widget(
            Paragraph::new(ln_lines).scroll((0, 0)),
            Rect {
                x: inner.x,
                y: inner.y,
                width: ln_width as u16,
                height: inner.height,
            },
        );
    }

    // Content area: only the visible window of lines is materialised, so
    // vertical scroll is 0. Horizontal scroll is still applied.
    let cx = inner.x + ln_width as u16;
    let cw = inner.width.saturating_sub(ln_width as u16);
    // When there is no gutter (ln_width == 0) but word-wrap is on, fall back to
    // ratatui's built-in Wrap — there is no drift risk without a parallel gutter
    // paragraph, so the pre-expansion path is skipped and ratatui handles it.
    // `content_scroll` is measured in visual rows when wrapping is enabled.
    // The first logical line can therefore be partially visible; pass its
    // intra-line offset to ratatui when there is no gutter to pre-expand.
    let wrap_scroll = if app.word_wrap && ln_width == 0 {
        leading_rows as u16
    } else {
        0
    };
    let mut para = Paragraph::new(content_lines).scroll((wrap_scroll, hscroll));
    if app.word_wrap && ln_width == 0 {
        para = para.wrap(Wrap { trim: false });
    }
    f.render_widget(
        para,
        Rect {
            x: cx,
            y: inner.y,
            width: cw,
            height: inner.height,
        },
    );

    app.content_area = inner;
    app.blame_area = blame_area;
    app.fold_gutter_rows = new_fold_gutter_rows;

    // Scrollbar is computed from the full pane area (area minus border).
    let full_inner_x = area.x + 1;
    let full_inner_y = area.y + 1;
    let full_inner_w = area.width.saturating_sub(2) as usize;
    let full_inner_h = area.height.saturating_sub(2) as usize;
    draw_content_scrollbar(
        f,
        app,
        full_inner_x,
        full_inner_y,
        full_inner_w,
        full_inner_h,
    );

    // ── Bottom-bar: single-line blame ─────────────────────────────────
    if line_blame_area.height > 0 {
        blame::draw_bottom_bar_blame(f, app, line_blame_area);
    }
}

/// Draws the frame for an inline image preview: the pane border, a blanked
/// region the terminal will draw the bitmap into (recorded as `app.image_area`),
/// and the text placeholder as a caption underneath. The bitmap is emitted
/// post-frame by `crate::graphics::render_overlay`.
fn draw_image_preview(f: &mut Frame, app: &mut App, area: Rect, block: Block) {
    let inner = block.inner(area);
    f.render_widget(block, area);
    app.content_area = inner;

    if inner.width == 0 || inner.height == 0 {
        app.image_area = Rect::default();
        return;
    }

    // Reserve the last rows for the caption (`app.content` holds the
    // `[image file — PNG, WxH, size]` placeholder built by the loader).
    let caption: Vec<Line> = app
        .content
        .iter()
        .filter(|l| !l.is_empty())
        .map(|l| Line::from(Span::styled(l.clone(), Style::default().fg(app.theme.dim))))
        .collect();
    let caption_h = (caption.len() as u16 + 1).min(inner.height / 2);

    let img_area = Rect {
        height: inner.height - caption_h,
        ..inner
    };
    let caption_area = Rect {
        y: inner.y + inner.height - caption_h,
        height: caption_h,
        ..inner
    };

    // Blank the bitmap region so ratatui owns those cells and erases any stale
    // glyphs from a previous view; the terminal then paints the image on top.
    f.render_widget(Clear, img_area);
    f.render_widget(
        Paragraph::new(caption).alignment(Alignment::Center),
        caption_area,
    );

    app.image_area = img_area;
}

/// Renders lines from precomputed styled spans (pretty JSON or CSV/TSV table view).
/// Returns (ln_width, gutter_lines, content_lines, fold_gutter_rows).
#[allow(clippy::type_complexity, clippy::too_many_arguments)]
fn render_preformatted_lines<'a>(
    app: &'a App,
    spans_slice: &'a [Vec<(Style, String)>],
    scroll: usize,
    visible_end: usize,
    show_ln: bool,
    in_file_search: Option<&'a InFileSearch>,
    sel: Option<((usize, usize), (usize, usize))>,
    sel_bg: ratatui::style::Color,
) -> (usize, Vec<Line<'a>>, Vec<Line<'a>>, Vec<(u16, usize)>) {
    let ln_style = Style::default().fg(app.theme.dim);
    let lw = app.line_count().to_string().len().max(1);
    let gutters: Vec<Line> = if show_ln {
        (scroll..visible_end)
            .map(|i| {
                Line::from(Span::styled(
                    format!("{:>width$} ", i + 1, width = lw),
                    ln_style,
                ))
            })
            .collect()
    } else {
        vec![]
    };
    let ln_w = if show_ln { lw + 1 } else { 0 };
    let lines: Vec<Line> = spans_slice[scroll..visible_end]
        .iter()
        .enumerate()
        .map(|(offset, spans)| {
            let logical_idx = scroll + offset;
            let regions_owned: Vec<(Style, String)> =
                spans.iter().map(|(s, t)| (*s, t.clone())).collect();
            if let Some(s) = in_file_search {
                Line::from(apply_search_to_regions(
                    &regions_owned,
                    logical_idx,
                    s,
                    &app.theme,
                ))
            } else if let Some(((sl, sc), (el, ec))) = sel {
                if logical_idx >= sl && logical_idx <= el {
                    let col_start = if logical_idx == sl { sc } else { 0 };
                    let col_end = if logical_idx == el { ec } else { usize::MAX };
                    Line::from(apply_selection(
                        &regions_owned,
                        col_start,
                        col_end,
                        sel_bg,
                        app.theme.is_monochrome(),
                    ))
                } else {
                    Line::from(
                        regions_owned
                            .iter()
                            .map(|(s, t)| Span::styled(t.clone(), *s))
                            .collect::<Vec<_>>(),
                    )
                }
            } else {
                Line::from(
                    regions_owned
                        .iter()
                        .map(|(s, t)| Span::styled(t.clone(), *s))
                        .collect::<Vec<_>>(),
                )
            }
        })
        .collect();
    (ln_w, gutters, lines, vec![])
}

#[cfg(test)]
#[path = "draw_test.rs"]
mod draw_tests;
