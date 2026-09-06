//! Read-only queries over the active content source for `App`.
//!
//! The content pane can be backed by several sources depending on what is open:
//! a memory-mapped [`VirtualFile`](crate::virtual_file::VirtualFile), an
//! in-memory `Vec<String>`, pretty-printed JSON, or rendered markdown spans.
//! When viewing a file at a specific git revision, the snapshot content is loaded
//! into the same `Vec<String>` and `highlighted` fields, so no separate source
//! is needed. These helpers (`line_count`, `line_text`, and friends) hide that
//! branching behind a single interface so the rest of the app can ask "how many
//! lines?" or "what is line N?" without knowing which source is active. They
//! never mutate state; callers that need scrolling or navigation build on top of
//! these read accessors.

use super::App;
use unicode_width::UnicodeWidthStr;

impl App {
    /// Returns the total number of lines in the current content source
    /// (plugin content, virtual file, raw content, or JSON pretty).
    pub fn line_count(&self) -> usize {
        if let Some(path) = &self.current_file {
            if let Some(lines) = self.plugin_content.get(path) {
                return lines.len();
            }
        }
        if self.is_json && self.show_pretty_json && !self.json_pretty_lines.is_empty() {
            self.json_pretty_lines.len()
        } else if self.is_csv && self.show_csv_table && !self.csv_table_lines.is_empty() {
            self.csv_table_lines.len()
        } else if let Some(vf) = &self.virtual_file {
            vf.line_count()
        } else {
            self.content.len()
        }
    }

    /// Rebuilds the visible JSONL rows after an object is expanded or
    /// collapsed, preserving the physical source-line mapping.
    pub(crate) fn rebuild_jsonl_display(&mut self) {
        if !self.is_jsonl {
            return;
        }
        let (display, map) = crate::jsonl::build_display(&self.jsonl_source, &self.jsonl_expanded);
        self.content = display;
        self.jsonl_display_map = map;
        self.json_path_map.clear();
        for (source_line, raw) in self.jsonl_source.iter().enumerate() {
            let paths = serde_json::from_str::<serde_json::Value>(raw)
                .map(|value| crate::json_path::build_path_map(&value))
                .unwrap_or_else(|_| vec![None]);
            if self.jsonl_expanded.contains(&source_line) {
                self.json_path_map.extend(paths);
            } else {
                self.json_path_map.push(None);
            }
        }
        self.highlighted = self.highlighter.highlight(
            &self.current_file.clone().unwrap_or_default(),
            &self.content,
        );
    }

    /// Returns the text of the 0-indexed line, consulting the active content
    /// source: plugin content, pretty JSON, CSV table, virtual file, or raw content vec.
    pub fn line_text(&self, index: usize) -> Option<&str> {
        if let Some(path) = &self.current_file {
            if let Some(lines) = self.plugin_content_text.get(path) {
                return lines.get(index).map(|s| s.as_str());
            }
        }
        if self.is_json && self.show_pretty_json && !self.json_pretty_text.is_empty() {
            self.json_pretty_text.get(index).map(|s| s.as_str())
        } else if self.is_csv && self.show_csv_table && !self.csv_table_text.is_empty() {
            self.csv_table_text.get(index).map(|s| s.as_str())
        } else if let Some(vf) = &self.virtual_file {
            vf.line_text(index)
        } else {
            self.content.get(index).map(|s| s.as_str())
        }
    }

    /// Returns the display width of line `index` in terminal columns.
    pub fn line_width(&self, index: usize) -> Option<usize> {
        if let Some(path) = &self.current_file {
            if let Some(lines) = self.plugin_content.get(path) {
                return lines
                    .get(index)
                    .map(|spans| spans.iter().map(|(_, text)| text.width()).sum());
            }
        }
        if let Some(vf) = &self.virtual_file {
            vf.line_width(index)
        } else {
            self.line_text(index)
                .map(unicode_width::UnicodeWidthStr::width)
        }
    }

    /// Syntax-highlights a slice of lines for the visible window, using the
    /// syntax already resolved when the file was opened (`current_syntax`)
    /// rather than re-detecting it from disk on every scroll redraw.
    pub fn highlight_lines(&self, lines: &[&str]) -> Vec<Vec<(ratatui::style::Style, String)>> {
        self.highlighter
            .highlight_range(self.current_syntax.as_deref(), lines)
    }

    /// Whether the diff should currently render in the side-by-side layout: the
    /// toggle is on, a diff is loaded, and the content pane is wide enough.
    pub fn diff_sbs_active(&self) -> bool {
        self.is_diff
            && self.diff_side_by_side
            && !self.diff_rows.is_empty()
            && self.content_area.width >= crate::diff::MIN_SIDE_BY_SIDE_WIDTH
    }

    /// Returns the number of **display** lines after folding/filtering. Equals
    /// `line_count()` when no folds/filters are active.
    pub fn display_line_count(&self) -> usize {
        if self.diff_sbs_active() {
            self.diff_rows.len()
        } else if self.filter_query.is_some() {
            self.filter_display_map.len()
        } else if self.fold_display_map.is_empty() {
            self.line_count()
        } else {
            self.fold_display_map.len()
        }
    }

    /// Maps a display-space line index to a physical file line index.
    pub fn display_to_physical(&self, display: usize) -> usize {
        if self.filter_query.is_some() {
            self.filter_display_map
                .get(display)
                .copied()
                .unwrap_or(display)
        } else if self.is_jsonl && !self.jsonl_display_map.is_empty() {
            self.jsonl_display_map
                .get(display)
                .copied()
                .unwrap_or(display)
        } else if self.fold_display_map.is_empty() {
            display
        } else {
            self.fold_display_map
                .get(display)
                .copied()
                .unwrap_or(display)
        }
    }

    /// Converts a physical line index to a display line index.
    /// When folding/filtering is inactive this is identity; when active it finds the
    /// position of `physical` in the display map.
    pub fn physical_to_display(&self, physical: usize) -> usize {
        if self.filter_query.is_some() {
            self.filter_display_map
                .iter()
                .position(|&p| p >= physical)
                .unwrap_or(self.filter_display_map.len().saturating_sub(1))
        } else if self.is_jsonl && !self.jsonl_display_map.is_empty() {
            self.jsonl_display_map
                .iter()
                .position(|&line| line >= physical)
                .unwrap_or(self.jsonl_display_map.len().saturating_sub(1))
        } else if self.fold_display_map.is_empty() {
            physical
        } else {
            // Find the first display line whose physical index is >= physical.
            self.fold_display_map
                .iter()
                .position(|&p| p >= physical)
                .unwrap_or(self.fold_display_map.len().saturating_sub(1))
        }
    }

    /// Rebuilds the filter display map for the current filter query, using cache if available.
    pub fn rebuild_filter_display_map(&mut self) {
        let query = match &self.filter_query {
            Some(q) if !q.is_empty() => q,
            _ => {
                self.filter_display_map.clear();
                return;
            }
        };

        let cache_key = (self.content_revision, query.clone());
        let cached = self.filter_cache.borrow().get(&cache_key).cloned();
        if let Some(map) = cached {
            self.filter_display_map = map;
            return;
        }

        // Perform case-insensitive search
        let query_lower = query.to_lowercase();
        let total = self.line_count();
        let mut map = Vec::new();
        for i in 0..total {
            if let Some(text) = self.line_text(i) {
                if text.to_lowercase().contains(&query_lower) {
                    map.push(i);
                }
            }
        }

        self.filter_cache
            .borrow_mut()
            .insert(cache_key, map.clone());
        self.filter_display_map = map;
    }
}

#[cfg(test)]
#[path = "content_query_test.rs"]
mod content_query_test;
