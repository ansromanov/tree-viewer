// Tests for active-line highlight logic in draw_content.
//
// Full rendering tests for draw_content are in content_test.rs.
// These tests cover the active-line highlight guard introduced by this PR.

use std::fs;
use std::path::PathBuf;
use std::sync::atomic::{AtomicUsize, Ordering};

use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use ratatui::backend::TestBackend;

use crate::app::{App, Focus};
use crate::config::Config;
use crate::ui::content::draw_content;

static COUNTER: AtomicUsize = AtomicUsize::new(0);

fn temp_tree() -> PathBuf {
    let n = COUNTER.fetch_add(1, Ordering::Relaxed);
    let dir = std::env::temp_dir().join(format!("tv_draw_test_{}_{n}", std::process::id()));
    fs::create_dir_all(&dir).unwrap();
    let long: String = (1..=20).map(|i| format!("line {i}\n")).collect();
    fs::write(dir.join("long.txt"), long).unwrap();
    let md = "# Title\n\nA long paragraph of markdown content that should be wrapped by the renderer when word wrap is enabled.\n";
    fs::write(dir.join("readme.md"), md).unwrap();
    dir.canonicalize().unwrap()
}

fn app_for(root: &std::path::Path) -> App {
    App::new(root.to_path_buf(), Config::default(), None, None).unwrap()
}

#[test]
fn active_line_initialises_to_zero_on_open() {
    let root = temp_tree();
    let mut app = app_for(&root);
    app.open_file(&root.join("long.txt"));
    assert_eq!(app.active_line, 0);
    fs::remove_dir_all(&root).ok();
}

#[test]
fn wrapped_draw_handles_a_long_single_line() {
    let root = temp_tree();
    let mut app = app_for(&root);
    app.content = vec!["x".repeat(500), "tail".into()];
    app.virtual_file = None;
    app.word_wrap = true;
    app.content_area = ratatui::layout::Rect::new(0, 0, 30, 8);
    let backend = TestBackend::new(30, 8);
    let mut terminal = ratatui::Terminal::new(backend).unwrap();
    terminal
        .draw(|frame| draw_content(frame, &mut app, frame.area()))
        .unwrap();
    assert!(app.content_scroll_max() > 0);
    fs::remove_dir_all(&root).ok();
}

#[test]
fn wrapped_plugin_content_scrolls_by_visual_row() {
    let root = temp_tree();
    let mut app = app_for(&root);
    let path = root.join("readme.md");
    let chars: String = (0..80).map(|i| char::from(b'a' + (i % 26) as u8)).collect();
    let rendered = format!("{chars}{chars}");
    app.current_file = Some(path.clone());
    app.plugin_content.insert(
        path.clone(),
        vec![vec![(ratatui::style::Style::default(), rendered.clone())]],
    );
    app.plugin_content_text.insert(path, vec![rendered]);
    app.word_wrap = true;

    let backend = TestBackend::new(20, 6);
    let mut terminal = ratatui::Terminal::new(backend).unwrap();
    terminal
        .draw(|frame| draw_content(frame, &mut app, frame.area()))
        .unwrap();
    app.set_content_scroll(1);
    terminal
        .draw(|frame| draw_content(frame, &mut app, frame.area()))
        .unwrap();

    // The first content row is the second wrapped row (18 columns wide inside
    // the bordered pane), not the first row of the logical markdown line.
    assert_eq!(terminal.backend().buffer()[(1, 1)].symbol(), "s");
    fs::remove_dir_all(&root).ok();
}

#[test]
fn active_line_saturates_at_last_line() {
    let root = temp_tree();
    let mut app = app_for(&root);
    app.open_file(&root.join("long.txt"));
    app.focus = Focus::Content;

    for _ in 0..200 {
        app.handle_key(KeyEvent::new(KeyCode::Char('j'), KeyModifiers::empty()));
    }
    let max = app.display_line_count().saturating_sub(1);
    assert_eq!(app.active_line, max);
    fs::remove_dir_all(&root).ok();
}

#[test]
fn active_line_highlight_guard_skipped_in_diff_mode() {
    let root = temp_tree();
    let mut app = app_for(&root);
    app.is_diff = true;
    assert!(app.is_diff);
    assert_eq!(app.active_line, 0);
    fs::remove_dir_all(&root).ok();
}

/// Renders `draw_content` into an 80x24 backend and reports whether any cell
/// carries the `active_line_bg` background — the marker left by the active-line
/// highlight.
fn renders_active_line_highlight(app: &mut App) -> bool {
    let active_bg = app.theme.active_line_bg;
    let backend = TestBackend::new(80, 24);
    let mut terminal = ratatui::Terminal::new(backend).unwrap();
    terminal
        .draw(|frame| draw_content(frame, app, frame.area()))
        .unwrap();
    terminal
        .backend()
        .buffer()
        .content()
        .iter()
        .any(|c| c.bg == active_bg)
}

#[test]
fn active_line_highlight_painted_for_normal_file() {
    let root = temp_tree();
    let mut app = app_for(&root);
    app.open_file(&root.join("long.txt"));
    assert!(
        renders_active_line_highlight(&mut app),
        "active line should be highlighted with active_line_bg"
    );
    fs::remove_dir_all(&root).ok();
}

#[test]
fn active_line_highlight_absent_in_diff_mode() {
    let root = temp_tree();
    let mut app = app_for(&root);
    app.open_file(&root.join("long.txt"));
    app.is_diff = true;
    assert!(
        !renders_active_line_highlight(&mut app),
        "diff mode must skip the active-line highlight"
    );
    fs::remove_dir_all(&root).ok();
}

/// Returns the (row, col) of every cell with the given background color.
fn cells_with_bg(app: &mut App, bg: ratatui::style::Color) -> Vec<(u16, u16)> {
    let backend = TestBackend::new(80, 24);
    let mut terminal = ratatui::Terminal::new(backend).unwrap();
    terminal
        .draw(|frame| draw_content(frame, app, frame.area()))
        .unwrap();
    let buf = terminal.backend().buffer();
    buf.content()
        .iter()
        .enumerate()
        .filter(|(_, c)| c.bg == bg)
        .map(|(i, _)| {
            let col = (i as u16) % 80;
            let row = (i as u16) / 80;
            (row, col)
        })
        .collect()
}

#[test]
fn active_line_full_width_highlight() {
    let root = temp_tree();
    let mut app = app_for(&root);
    app.open_file(&root.join("long.txt"));
    let active_bg = app.theme.active_line_bg;
    let cells = cells_with_bg(&mut app, active_bg);
    assert!(!cells.is_empty(), "active line must have highlighted cells");
    // All active-bg cells must be on the same row (line 0 since active_line=0)
    let rows: std::collections::BTreeSet<u16> = cells.iter().map(|(r, _)| *r).collect();
    assert_eq!(
        rows.len(),
        1,
        "active line highlight must be on a single row"
    );
    // The highlight must span at least 50 columns (content area is ~77 wide at 80-2 border - gutter)
    let min_col = cells.iter().map(|(_, c)| c).min().unwrap();
    let max_col = cells.iter().map(|(_, c)| c).max().unwrap();
    assert!(
        max_col - min_col >= 50,
        "active line highlight must span more than 50 columns, got {}-{}={}",
        max_col,
        min_col,
        max_col - min_col
    );
    // Verify gutter cells (col < ln_width ~4) also have active_bg
    let gutter_cells: Vec<_> = cells.iter().filter(|(_, c)| *c < 5).collect();
    assert!(
        !gutter_cells.is_empty(),
        "gutter must also be highlighted with active_line_bg"
    );
    fs::remove_dir_all(&root).ok();
}

#[test]
fn active_line_caret_in_gutter() {
    let root = temp_tree();
    let mut app = app_for(&root);
    app.open_file(&root.join("long.txt"));
    let active_bg = app.theme.active_line_bg;
    let accent = app.theme.accent;
    let backend = TestBackend::new(80, 24);
    let mut terminal = ratatui::Terminal::new(backend).unwrap();
    terminal
        .draw(|frame| draw_content(frame, &mut app, frame.area()))
        .unwrap();
    let buf = terminal.backend().buffer();
    // Gutter is at the left of the content pane (column ~1 after border).
    // The active line's gutter should have accent foreground.
    let has_accent_in_gutter = buf.content().iter().enumerate().any(|(i, c)| {
        let col = (i as u16) % 80;
        let _row = (i as u16) / 80;
        col < 5 && c.fg == accent
    });
    assert!(
        has_accent_in_gutter,
        "gutter on active line must use accent foreground"
    );
    // The active line gutter must also have active_line_bg background
    let has_active_bg_in_gutter = buf.content().iter().enumerate().any(|(i, c)| {
        let col = (i as u16) % 80;
        let _row = (i as u16) / 80;
        col < 5 && c.bg == active_bg
    });
    assert!(
        has_active_bg_in_gutter,
        "gutter on active line must have active_line_bg background"
    );
    fs::remove_dir_all(&root).ok();
}

#[test]
fn active_line_moves_with_down_key() {
    let root = temp_tree();
    let mut app = app_for(&root);
    app.open_file(&root.join("long.txt"));
    app.focus = Focus::Content;
    // Move down one line
    app.handle_key(KeyEvent::new(KeyCode::Char('j'), KeyModifiers::empty()));
    assert_eq!(app.active_line, 1);
    // Render and check the highlight is now on row for line 1
    let active_bg = app.theme.active_line_bg;
    let c1 = cells_with_bg(&mut app, active_bg);
    // The highlighted row shouldn't be the same as for line 0
    assert!(!c1.is_empty(), "highlight must exist after moving down");
    fs::remove_dir_all(&root).ok();
}

#[test]
fn selection_highlight_survives_on_active_line() {
    let root = temp_tree();
    let mut app = app_for(&root);
    app.open_file(&root.join("long.txt"));
    // Mouse drag on the active line: anchor and active both on line 0, which
    // is also the cursor line (active_line = 0 after open).
    app.selection = Some(crate::selection::TextSelection {
        anchor: (0, 0),
        active: (0, 6),
    });
    let sel_bg = app.theme.selection_bg;
    let cells = cells_with_bg(&mut app, sel_bg);
    assert!(
        !cells.is_empty(),
        "selection on the active line must render with selection_bg, \
         not be clobbered by the active-line highlight"
    );
    fs::remove_dir_all(&root).ok();
}

#[test]
fn active_line_highlight_returns_when_selection_elsewhere() {
    let root = temp_tree();
    let mut app = app_for(&root);
    app.open_file(&root.join("long.txt"));
    app.focus = Focus::Content;
    // Cursor on line 2; a lingering selection on line 5 only.
    app.handle_key(KeyEvent::new(KeyCode::Char('j'), KeyModifiers::empty()));
    app.handle_key(KeyEvent::new(KeyCode::Char('j'), KeyModifiers::empty()));
    app.selection = Some(crate::selection::TextSelection {
        anchor: (5, 0),
        active: (5, 4),
    });
    assert!(
        renders_active_line_highlight(&mut app),
        "active-line highlight must still paint when the selection does not cover it"
    );
    fs::remove_dir_all(&root).ok();
}

#[test]
fn active_line_highlight_returns_when_selection_ends_at_column_zero() {
    let root = temp_tree();
    let mut app = app_for(&root);
    app.open_file(&root.join("long.txt"));
    app.focus = Focus::Content;
    // Cursor on line 2. Selection spans lines 0-2 but ends at column 0 on
    // line 2, so apply_selection's half-open range highlights nothing there -
    // the active-line background must still paint on line 2.
    app.handle_key(KeyEvent::new(KeyCode::Char('j'), KeyModifiers::empty()));
    app.handle_key(KeyEvent::new(KeyCode::Char('j'), KeyModifiers::empty()));
    app.selection = Some(crate::selection::TextSelection {
        anchor: (0, 3),
        active: (2, 0),
    });
    assert!(
        renders_active_line_highlight(&mut app),
        "active-line highlight must paint when the selection's end-column-0 line \
         has no actual selection highlight"
    );
    fs::remove_dir_all(&root).ok();
}

/// Renders `draw_content` and returns the flattened text of the buffer.
fn render_to_string(app: &mut App) -> String {
    let backend = TestBackend::new(80, 24);
    let mut terminal = ratatui::Terminal::new(backend).unwrap();
    terminal
        .draw(|frame| draw_content(frame, app, frame.area()))
        .unwrap();
    terminal
        .backend()
        .buffer()
        .content()
        .iter()
        .map(|c| c.symbol())
        .collect()
}

#[test]
fn plugin_content_is_rendered_for_current_file() {
    let root = temp_tree();
    let mut app = app_for(&root);
    let path = root.join("long.txt");
    app.open_file(&path);
    app.plugin_content.insert(
        path.clone(),
        vec![vec![(
            ratatui::style::Style::default(),
            "PLUGIN_RENDERED_MARKER".to_string(),
        )]],
    );
    app.plugin_content_text
        .insert(path, vec!["PLUGIN_RENDERED_MARKER".to_string()]);
    let out = render_to_string(&mut app);
    assert!(
        out.contains("PLUGIN_RENDERED_MARKER"),
        "plugin content must take precedence in the content pane"
    );
    fs::remove_dir_all(&root).ok();
}

#[test]
fn word_wrap_without_line_numbers_still_wraps() {
    // Regression: when ln_width==0 (no line numbers, no blame, no folds),
    // word wrap must still visually wrap long lines — the pre-expansion path
    // only runs when ln_width>0, so ratatui Wrap is the fallback.
    let root = temp_tree();
    let long_line: String = "x".repeat(160);
    let path = root.join("wrap_no_ln.txt");
    fs::write(&path, format!("{long_line}\n")).unwrap();
    let mut app = app_for(&root);
    app.open_file(&path);
    app.show_line_numbers = false;
    app.word_wrap = true;
    // Render into 80x24 buffer.
    let backend = ratatui::backend::TestBackend::new(80, 24);
    let mut terminal = ratatui::Terminal::new(backend).unwrap();
    terminal
        .draw(|frame| draw_content(frame, &mut app, frame.area()))
        .unwrap();
    let buf = terminal.backend().buffer();
    let row = |y: u16| -> String {
        let w = buf.area.width as usize;
        let start = y as usize * w;
        buf.content()[start..start + w]
            .iter()
            .map(|c| c.symbol())
            .collect()
    };
    let r1 = row(1);
    let r2 = row(2);
    assert!(
        r1.trim_end().contains('x'),
        "row 1 must contain content, got: {r1:?}"
    );
    assert!(
        r2.trim_end().contains('x'),
        "row 2 must also have content (word wrap), got: {r2:?}"
    );
    fs::remove_dir_all(&root).ok();
}

#[test]
fn plugin_content_no_line_number_gutter() {
    // Plugin-rendered content should not show line-number gutter even when
    // show_line_numbers=true, because the content is rendered and indices
    // don't map to source.
    let root = temp_tree();
    let mut app = app_for(&root);
    let path = root.join("long.txt");
    app.open_file(&path);
    app.show_line_numbers = true;
    // Install plugin content.
    app.plugin_content.insert(
        path.clone(),
        vec![
            vec![(
                ratatui::style::Style::default(),
                "PLUGIN LINE 1".to_string(),
            )],
            vec![(
                ratatui::style::Style::default(),
                "PLUGIN LINE 2".to_string(),
            )],
        ],
    );
    app.plugin_content_text.insert(
        path,
        vec!["PLUGIN LINE 1".to_string(), "PLUGIN LINE 2".to_string()],
    );
    // line_prefix_width() should be 0 for plugin content.
    assert_eq!(
        app.line_prefix_width(),
        0,
        "plugin content must have no line-number gutter width"
    );
    // Render and verify no line numbers appear before plugin content.
    let out = render_to_string(&mut app);
    assert!(
        out.contains("PLUGIN LINE 1"),
        "plugin content must be rendered"
    );
    assert!(
        !out.contains("1 PLUGIN"),
        "plugin content must not have line numbers in gutter"
    );
    fs::remove_dir_all(&root).ok();
}
#[test]
fn empty_state_shows_orientation_hint_when_no_file_open() {
    let root = temp_tree();
    let mut app = app_for(&root);
    app.current_file = None;
    app.content = Vec::new();
    app.highlighted = Vec::new();
    app.virtual_file = None;
    let out = render_to_string(&mut app);
    assert!(
        out.contains("to search") && out.contains("to open a file"),
        "empty content pane should show the orientation hint; got: {out:?}"
    );
    fs::remove_dir_all(&root).ok();
}

/// #304 discoverability: the first-run hint must also point at `?` for help,
/// since the status bar no longer renders keybinding hints on its own.
#[test]
fn empty_state_hint_mentions_help_key() {
    let root = temp_tree();
    let mut app = app_for(&root);
    app.current_file = None;
    app.content = Vec::new();
    app.highlighted = Vec::new();
    app.virtual_file = None;
    let out = render_to_string(&mut app);
    assert!(
        out.contains("for help"),
        "empty content pane hint should point at the help key; got: {out:?}"
    );
    fs::remove_dir_all(&root).ok();
}

#[test]
fn empty_state_hint_survives_word_wrap_with_line_number_gutter() {
    // Regression: the hint is appended to content_lines only. wrap_content's
    // zip(gutters, content) stops at the shorter side, so without a matching
    // blank gutter row the hint was silently dropped whenever word wrap was
    // on and the line-number gutter was showing.
    let root = temp_tree();
    let mut app = app_for(&root);
    app.current_file = None;
    app.content = Vec::new();
    app.highlighted = Vec::new();
    app.virtual_file = None;
    app.word_wrap = true;
    app.show_line_numbers = true;
    let out = render_to_string(&mut app);
    assert!(
        out.contains("to search") && out.contains("to open a file"),
        "orientation hint must not be dropped by word-wrap's gutter/content zip; got: {out:?}"
    );
    fs::remove_dir_all(&root).ok();
}

#[test]
fn draw_content_with_show_blame_does_not_panic() {
    let root = temp_tree();
    let mut app = app_for(&root);
    app.open_file(&root.join("long.txt"));
    app.show_blame = true;
    let backend = TestBackend::new(80, 24);
    let mut terminal = ratatui::Terminal::new(backend).unwrap();
    terminal
        .draw(|frame| draw_content(frame, &mut app, frame.area()))
        .unwrap();
    fs::remove_dir_all(&root).ok();
}

#[test]
fn draw_content_with_show_line_blame_does_not_panic() {
    let root = temp_tree();
    let mut app = app_for(&root);
    app.open_file(&root.join("long.txt"));
    app.show_line_blame = true;
    let backend = TestBackend::new(80, 24);
    let mut terminal = ratatui::Terminal::new(backend).unwrap();
    terminal
        .draw(|frame| draw_content(frame, &mut app, frame.area()))
        .unwrap();
    fs::remove_dir_all(&root).ok();
}

#[test]
fn draw_content_reserves_a_blanked_region_and_shows_the_caption_for_an_inline_image() {
    let root = temp_tree();
    let mut app = app_for(&root);
    let path = root.join("pixel.png");
    let png_1x1 = {
        let img = image::RgbImage::from_pixel(1, 1, image::Rgb([1, 2, 3]));
        let mut buf = Vec::new();
        image::DynamicImage::ImageRgb8(img)
            .write_to(&mut std::io::Cursor::new(&mut buf), image::ImageFormat::Png)
            .unwrap();
        buf
    };
    fs::write(&path, &png_1x1).unwrap();
    app.current_file = Some(path);
    app.content = vec![
        "[image file — PNG, 1x1, 70 B]".into(),
        "".into(),
        "press o to open with the system default app".into(),
    ];
    app.content_image = crate::graphics::ContentImage::from_bytes(&png_1x1);
    assert!(app.content_image.is_some(), "precondition: image decodes");

    let out = render_to_string(&mut app);

    assert_ne!(app.image_area, ratatui::layout::Rect::default());
    assert!(
        app.image_area.height < app.content_area.height,
        "the caption rows must be excluded from the reserved image region"
    );
    assert!(
        out.contains("press o to open"),
        "caption rendered underneath the reserved region: {out}"
    );

    fs::remove_dir_all(&root).ok();
}

#[test]
fn draw_content_renders_csv_table_view() {
    let root = temp_tree();
    let mut app = app_for(&root);
    let f = root.join("table.csv");
    std::fs::write(&f, "heading1,heading2\nval1,val2\n").unwrap();
    app.open_file(&f);
    assert!(app.is_csv);
    assert!(app.show_csv_table);

    let out = render_to_string(&mut app);
    assert!(out.contains('┌'), "table top border rendered: {out}");
    assert!(out.contains("heading1"), "header rendered: {out}");
    assert!(out.contains("val1"), "data cell rendered: {out}");

    fs::remove_dir_all(&root).ok();
}
