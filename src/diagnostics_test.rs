// Tests for diagnostic report generation and configuration diffing.
use super::*;

use std::fs;

use crate::config::TelemetryConfig;

fn fixture_root() -> tempfile::TempDir {
    let tmp = tempfile::tempdir().unwrap();
    fs::create_dir(tmp.path().join("sub")).unwrap();
    fs::write(tmp.path().join("secret_notes.rs"), "fn main() {}\n").unwrap();
    fs::write(tmp.path().join("sub").join("inner.txt"), "hello\n").unwrap();
    tmp
}

fn app_for(root: &std::path::Path) -> App {
    App::new(root.to_path_buf(), Config::default(), None, None).unwrap()
}

#[test]
fn collect_counts_workspace_shape() {
    let tmp = fixture_root();
    let app = app_for(tmp.path());
    let report = DiagnosticReport::collect(&app);

    assert_eq!(report.app_version, env!("CARGO_PKG_VERSION"));
    assert_eq!(report.os, std::env::consts::OS);
    // Only the root's direct children are visible while `sub/` is collapsed.
    assert_eq!(report.tree_nodes, 2);
    assert_eq!(report.tree_files, 1);
    assert_eq!(report.tree_dirs, 1);
    assert_eq!(report.tree_files + report.tree_dirs, report.tree_nodes);
    assert!(!report.file_open);
    // Bundled plugins are seeded into the config at construction time.
    assert_eq!(report.plugin_count, app.config.plugins.len());
    assert!(!report.telemetry_enabled);
}

#[test]
fn collect_reports_open_file_facts_without_its_name() {
    let tmp = fixture_root();
    let mut app = app_for(tmp.path());
    app.current_file = Some(tmp.path().join("secret_notes.rs"));
    app.file_encoding = Some("UTF-8".into());
    app.file_line_ending = Some("LF".into());
    app.current_syntax = Some("Rust".into());

    let report = DiagnosticReport::collect(&app);
    assert!(report.file_open);
    assert_eq!(report.file_extension.as_deref(), Some("rs"));
    assert_eq!(report.file_size_bytes, Some(13));
    assert_eq!(report.file_encoding.as_deref(), Some("UTF-8"));
    assert_eq!(report.file_syntax.as_deref(), Some("Rust"));
    assert!(!report.file_is_csv);
}

#[test]
fn serialized_report_never_contains_paths_or_names() {
    let tmp = fixture_root();
    let mut app = app_for(tmp.path());
    app.current_file = Some(tmp.path().join("secret_notes.rs"));

    let report = DiagnosticReport::collect(&app);
    for rendered in [
        serde_json::to_string(&report).unwrap(),
        report.to_markdown(),
    ] {
        let root = tmp.path().to_string_lossy().to_string();
        assert!(!rendered.contains(&root), "workspace root leaked");
        assert!(!rendered.contains("secret_notes"), "file name leaked");
        if let Ok(home) = std::env::var("HOME") {
            if !home.is_empty() {
                assert!(!rendered.contains(&home), "home dir leaked");
            }
        }
    }
}

#[test]
fn changed_config_paths_reports_paths_without_values() {
    let mut cfg = Config::default();
    cfg.tree.show_hidden = true;
    cfg.telemetry = TelemetryConfig {
        enabled: true,
        notice_shown: false,
    };
    cfg.plugins.insert(
        "my-secret-plugin".into(),
        crate::plugin::PluginEntry::default(),
    );

    let paths = changed_config_paths(&cfg);
    assert!(paths.contains(&"tree.show_hidden".to_string()));
    assert!(paths.contains(&"telemetry.enabled".to_string()));
    assert!(paths.contains(&"plugins".to_string()));
    assert!(
        !paths.iter().any(|p| p.contains("my-secret-plugin")),
        "plugin names must never appear"
    );
}

#[test]
fn changed_config_paths_empty_for_default_config() {
    assert!(changed_config_paths(&Config::default()).is_empty());
}

#[test]
fn save_writes_markdown_under_state_dir() {
    let _guard = crate::session::STATE_DIR_ENV_LOCK
        .lock()
        .unwrap_or_else(|e| e.into_inner());
    let state = tempfile::tempdir().unwrap();
    std::env::set_var("MANTIS_STATE_DIR", state.path());

    let tmp = fixture_root();
    let app = app_for(tmp.path());
    let report = DiagnosticReport::collect(&app);
    let path = report.save().unwrap();

    assert!(path.starts_with(state.path().join("bug-reports")));
    let body = fs::read_to_string(&path).unwrap();
    assert!(body.contains("## mantis diagnostic report"));
    assert!(body.contains(env!("CARGO_PKG_VERSION")));
    std::env::remove_var("MANTIS_STATE_DIR");
}

#[test]
fn to_markdown_lists_all_sections() {
    let tmp = fixture_root();
    let app = app_for(tmp.path());
    let md = DiagnosticReport::collect(&app).to_markdown();
    for needle in [
        "- **app version**:",
        "- **target**:",
        "- **release date**:",
        "- **os**:",
        "- **arch**:",
        "- **wsl**:",
        "- **terminal**:",
        "- **size**:",
        "- **workspace**:",
        "- **open file**:",
        "- **theme**:",
        "- **config overrides**:",
        "- **plugins**:",
        "- **telemetry**:",
    ] {
        assert!(md.contains(needle), "missing section {needle}");
    }
}

#[test]
fn collect_includes_bug_report_body() {
    let tmp = fixture_root();
    let mut app = app_for(tmp.path());
    let mut state = crate::search::BugReportState::new(String::new());
    state.text = vec!["Hello bug".to_string(), "Line 2".to_string()];
    app.bug_report = Some(state);

    let report = DiagnosticReport::collect(&app);
    assert_eq!(report.body, "Hello bug\nLine 2");
    assert!(!report.target_triple.is_empty());

    let md = report.to_markdown();
    assert!(md.contains("## bug report body\n\nHello bug\nLine 2\n\n"));
}
