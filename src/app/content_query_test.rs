// Tests for the plugin-content branches of the content-source queries:
// `line_count` and `line_text` must read from `plugin_content` /
// `plugin_content_text` when the current file has plugin-provided content.

use std::fs;
use std::path::PathBuf;
use std::sync::atomic::{AtomicUsize, Ordering};

use ratatui::style::Style;

use crate::app::App;
use crate::config::Config;

static COUNTER: AtomicUsize = AtomicUsize::new(0);

fn temp_root() -> PathBuf {
    let n = COUNTER.fetch_add(1, Ordering::Relaxed);
    let dir = std::env::temp_dir().join(format!("tv_cquery_test_{}_{n}", std::process::id()));
    fs::create_dir_all(&dir).unwrap();
    fs::write(dir.join("doc.md"), "placeholder\n").unwrap();
    dir.canonicalize().unwrap()
}

fn app_for(root: &std::path::Path) -> App {
    App::new(root.to_path_buf(), Config::default(), None, None).unwrap()
}

fn seed_plugin(app: &mut App, path: PathBuf, lines: &[&str]) {
    let rendered: Vec<Vec<(Style, String)>> = lines
        .iter()
        .map(|l| vec![(Style::default(), l.to_string())])
        .collect();
    let text: Vec<String> = lines.iter().map(|l| l.to_string()).collect();
    app.plugin_content_text.insert(path.clone(), text);
    app.plugin_content.insert(path.clone(), rendered);
    app.current_file = Some(path);
}

#[test]
fn rebuild_filter_display_map_filters_correctly() {
    let root = temp_root();
    let mut app = app_for(&root);
    app.virtual_file = None;
    app.content = vec![
        "error message".to_string(),
        "info msg".to_string(),
        "debug warning".to_string(),
        "another error".to_string(),
    ];
    app.filter_query = Some("error".to_string());
    app.rebuild_filter_display_map();
    assert_eq!(app.display_line_count(), 2);
    assert_eq!(app.display_to_physical(0), 0);
    assert_eq!(app.display_to_physical(1), 3);
    fs::remove_dir_all(&root).ok();
}

#[test]
fn line_count_reads_plugin_content() {
    let root = temp_root();
    let mut app = app_for(&root);
    let path = root.join("doc.md");
    seed_plugin(&mut app, path, &["a", "b", "c"]);
    assert_eq!(app.line_count(), 3);
    fs::remove_dir_all(&root).ok();
}

#[test]
fn line_text_reads_plugin_content_text() {
    let root = temp_root();
    let mut app = app_for(&root);
    let path = root.join("doc.md");
    seed_plugin(&mut app, path, &["first", "second"]);
    assert_eq!(app.line_text(0), Some("first"));
    assert_eq!(app.line_text(1), Some("second"));
    assert_eq!(app.line_text(2), None);
    fs::remove_dir_all(&root).ok();
}

#[test]
fn line_count_ignores_plugin_content_for_other_file() {
    let root = temp_root();
    let mut app = app_for(&root);
    // Plugin content keyed by a different path must not affect the open file.
    let other = root.join("other.md");
    let rendered: Vec<Vec<(Style, String)>> = (0..5)
        .map(|i| vec![(Style::default(), format!("l{i}"))])
        .collect();
    app.plugin_content.insert(other.clone(), rendered);
    app.plugin_content_text
        .insert(other, (0..5).map(|i| format!("l{i}")).collect());
    app.current_file = Some(root.join("doc.md"));
    // Falls through to the normal content source, not the 5-line plugin entry
    // keyed by `other.md`.
    assert_ne!(app.line_count(), 5);
    fs::remove_dir_all(&root).ok();
}

#[test]
fn line_count_md_file_uses_virtual_file_not_builtin_markdown() {
    let root = temp_root();
    let mut app = app_for(&root);
    let path = root.join("doc.md");
    fs::write(&path, "line1\nline2\nline3\n").unwrap();
    app.open_file(&path);
    // Without the built-in markdown renderer, .md files fall through to
    // VirtualFile, so line_count reflects the raw file.
    assert_eq!(app.line_count(), 3);
    fs::remove_dir_all(&root).ok();
}

#[test]
fn highlight_lines_uses_current_syntax_not_a_path_arg() {
    // highlight_lines was changed to route through `self.current_syntax`
    // (resolved once at file-open time) instead of taking a path, so
    // repeated scroll redraws don't re-open the file to sniff its syntax.
    let root = temp_root();
    let mut app = app_for(&root);
    app.current_syntax = Some("Rust".to_string());
    let result = app.highlight_lines(&["fn main() {"]);
    assert!(
        result[0].len() > 1,
        "Rust syntax should produce multiple styled spans, got {}",
        result[0].len()
    );
    fs::remove_dir_all(&root).ok();
}

#[test]
fn highlight_lines_none_current_syntax_is_plain_text() {
    let root = temp_root();
    let mut app = app_for(&root);
    app.current_syntax = None;
    let result = app.highlight_lines(&["fn main() {"]);
    assert_eq!(
        result[0].len(),
        1,
        "no current_syntax should fall back to a single plain-text span"
    );
    fs::remove_dir_all(&root).ok();
}

#[test]
fn content_query_works_after_opening_file() {
    let root = temp_root();
    let mut app = app_for(&root);
    let f = root.join("test.txt");
    std::fs::write(&f, "line one\nline two\n").unwrap();
    app.open_file(&f);
    assert_eq!(app.line_count(), 2);
    assert_eq!(app.line_text(0), Some("line one"));
    assert_eq!(app.line_text(1), Some("line two"));
    assert!(app.line_text(2).is_none());
    fs::remove_dir_all(&root).ok();
}

#[test]
fn jsonl_display_map_points_expanded_rows_to_source_line() {
    let root = temp_root();
    let mut app = app_for(&root);
    app.virtual_file = None;
    app.is_jsonl = true;
    app.jsonl_source = vec![r#"{"nested":{"ok":true}}"#.into()];
    app.jsonl_expanded.insert(0);
    app.rebuild_jsonl_display();
    assert!(app.line_count() > 1);
    assert_eq!(app.display_to_physical(0), 0);
    assert!(app.line_text(1).is_some());
    fs::remove_dir_all(&root).ok();
}

#[test]
fn content_query_csv_table_view_and_toggle() {
    let root = temp_root();
    let mut app = app_for(&root);
    let f = root.join("data.csv");
    std::fs::write(&f, "city,pop\nParis,2100000\nTokyo,14000000\n").unwrap();
    app.open_file(&f);

    assert!(app.is_csv);
    assert!(app.show_csv_table);
    // Table has top border, header row, middle border, 2 data rows, bottom border = 6 lines
    assert_eq!(app.line_count(), 6);
    assert!(app.line_text(0).unwrap().starts_with('┌'));
    assert!(app.line_text(1).unwrap().contains("city"));
    assert!(app.line_text(3).unwrap().contains("Paris"));

    // Toggle off table view
    app.show_csv_table = false;
    assert_eq!(app.line_count(), 3);
    assert_eq!(app.line_text(0), Some("city,pop"));
    assert_eq!(app.line_text(1), Some("Paris,2100000"));

    fs::remove_dir_all(&root).ok();
}

#[test]
fn jsonl_display_rebuild_keeps_path_map_aligned() {
    let root = temp_root();
    let mut app = app_for(&root);
    app.virtual_file = None;
    app.is_jsonl = true;
    app.jsonl_source = vec![r#"{"spec":{"image":"app"}}"#.into()];
    app.jsonl_expanded.insert(0);
    app.rebuild_jsonl_display();
    assert_eq!(app.json_path_map.len(), app.content.len());
    assert!(app
        .json_path_map
        .iter()
        .flatten()
        .any(|p| p == ".spec.image"));
    fs::remove_dir_all(&root).ok();
}

#[test]
fn line_width_uses_rendered_plugin_lines_over_virtual_file() {
    let root = temp_root();
    let mut app = app_for(&root);
    let path = root.join("doc.md");
    app.current_file = Some(path.clone());
    app.virtual_file = crate::virtual_file::VirtualFile::open(&path);
    app.plugin_content.insert(
        path,
        vec![vec![(
            Style::default(),
            "rendered markdown line".to_string(),
        )]],
    );

    assert_eq!(app.line_width(0), Some("rendered markdown line".len()));
    fs::remove_dir_all(&root).ok();
}
