use super::*;

use std::fs;

/// Build a minimal snapshot for tests that don't need a real App.
fn test_snapshot() -> SessionSnapshot {
    SessionSnapshot {
        app_version: env!("CARGO_PKG_VERSION"),
        os: std::env::consts::OS,
        arch: std::env::consts::ARCH,
        terminal: std::env::var("TERM").unwrap_or_default(),
        os_version: None,
        wsl: false,
        term_program: None,
        term_program_version: None,
        colorterm: None,
        windows_terminal: false,
        ssh_session: false,
        terminal_size: None,
        tree_nodes: 0,
        tree_files: 0,
        tree_dirs: 0,
        tree_max_depth: 0,
        expanded_dirs: 0,
        tree_filter_active: false,
        walk_errors: 0,
        git_repo: false,
        git_mode: false,
        file_open: false,
        file_extension: None,
        file_size_bytes: None,
        file_line_count: None,
        file_encoding: None,
        file_line_ending: None,
        file_syntax: None,
        file_is_json: false,
        file_is_csv: false,
        file_is_diff: false,
        file_uses_mmap: false,
        theme: None,
        plugin_count: 0,
        telemetry_enabled: true,
    }
}

/// Find the active (most recent) events-*.jsonl file in `dir`.
fn active_file(dir: &std::path::Path) -> std::path::PathBuf {
    let mut files: Vec<_> = fs::read_dir(dir)
        .unwrap()
        .flatten()
        .map(|e| e.path())
        .filter(|p| {
            p.file_name()
                .and_then(|n| n.to_str())
                .is_some_and(|n| n.starts_with("events-") && n.ends_with(".jsonl"))
        })
        .collect();
    files.sort();
    files.into_iter().next().unwrap()
}

fn read_lines(dir: &std::path::Path) -> Vec<serde_json::Value> {
    let path = active_file(dir);
    let raw = fs::read_to_string(path).unwrap();
    raw.lines()
        .map(|l| serde_json::from_str(l).unwrap())
        .collect()
}

#[test]
fn disabled_handle_is_noop() {
    let t = Telemetry::disabled();
    assert!(!t.is_enabled());
    t.record(TelemetryEvent::ActionInvoked {
        action: "quit",
        source: ActionSource::Palette,
    });
    // Dropping must not panic or create anything; nothing observable to
    // assert beyond the absence of a writer thread (inner is None).
    assert!(t.inner.is_none());
    drop(t);
}

#[test]
fn new_disabled_creates_no_state_files() {
    let _guard = crate::session::STATE_DIR_ENV_LOCK
        .lock()
        .unwrap_or_else(|e| e.into_inner());
    let tmp = tempfile::tempdir().unwrap();
    std::env::set_var("MANTIS_STATE_DIR", tmp.path());
    let t = Telemetry::new(false);
    assert!(!t.is_enabled());
    drop(t);
    assert!(!tmp.path().join("telemetry").exists());
    std::env::remove_var("MANTIS_STATE_DIR");
}

#[test]
fn new_enabled_writes_under_state_dir_telemetry() {
    let _guard = crate::session::STATE_DIR_ENV_LOCK
        .lock()
        .unwrap_or_else(|e| e.into_inner());
    let tmp = tempfile::tempdir().unwrap();
    std::env::set_var("MANTIS_STATE_DIR", tmp.path());
    let t = Telemetry::new(true);
    assert!(t.is_enabled());
    drop(t); // joins the writer, flushing buffered events
    let telemetry_dir = tmp.path().join("telemetry");
    assert!(telemetry_dir.exists());
    assert!(fs::read_dir(&telemetry_dir).unwrap().flatten().any(|e| e
        .file_name()
        .to_str()
        .is_some_and(|n| n.starts_with("events-") && n.ends_with(".jsonl"))));
    std::env::remove_var("MANTIS_STATE_DIR");
}

#[test]
fn session_lifecycle_and_action_events_round_trip() {
    let tmp = tempfile::tempdir().unwrap();
    let t = Telemetry::with_dir(tmp.path().to_path_buf());
    t.record_session_start(test_snapshot());
    t.record(TelemetryEvent::ActionInvoked {
        action: "toggle_hidden",
        source: ActionSource::Palette,
    });
    drop(t);

    let lines = read_lines(tmp.path());
    assert_eq!(lines.len(), 3);
    assert_eq!(lines[0]["event"], "session_start");
    assert_eq!(lines[0]["app_version"], env!("CARGO_PKG_VERSION"));
    assert_eq!(lines[0]["os"], std::env::consts::OS);
    assert_eq!(lines[1]["event"], "action_invoked");
    assert_eq!(lines[1]["action"], "toggle_hidden");
    assert_eq!(lines[1]["source"], "palette");
    assert_eq!(lines[2]["event"], "session_end");
    assert_eq!(lines[2]["events_dropped"], 0);
    for line in &lines {
        assert!(line["ts_ms"].is_u64(), "every event is stamped: {line}");
    }
}

#[test]
fn events_contain_only_whitelisted_keys() {
    let tmp = tempfile::tempdir().unwrap();
    let t = Telemetry::with_dir(tmp.path().to_path_buf());
    t.record_session_start(test_snapshot());
    t.record(TelemetryEvent::ActionInvoked {
        action: "help",
        source: ActionSource::Palette,
    });
    drop(t);

    let allowed: Vec<&str> = [
        // Common.
        "event",
        "ts_ms",
        // SessionStart.
        "app_version",
        "os",
        "arch",
        "terminal",
        "os_version",
        "wsl",
        "term_program",
        "term_program_version",
        "colorterm",
        "windows_terminal",
        "ssh_session",
        "terminal_size",
        "tree_nodes",
        "tree_files",
        "tree_dirs",
        "tree_max_depth",
        "expanded_dirs",
        "tree_filter_active",
        "walk_errors",
        "git_repo",
        "git_mode",
        "file_open",
        "file_extension",
        "file_size_bytes",
        "file_line_count",
        "file_encoding",
        "file_line_ending",
        "file_syntax",
        "file_is_json",
        "file_is_csv",
        "file_is_diff",
        "file_uses_mmap",
        "theme",
        "plugin_count",
        "telemetry_enabled",
        // SessionEnd.
        "duration_s",
        "events_dropped",
        // ActionInvoked.
        "action",
        "source",
        // OverlayOpened.
        "kind",
        // FeatureUsed.
        "feature",
        // PluginToggled.
        "enabled",
        // PerfSpan.
        "span",
        "duration_bucket",
        // ErrorOccurred.
        "module",
        // FileOpened.
        "size_bucket",
        "source_kind",
        "encoding",
        "is_binary",
    ]
    .to_vec();
    for line in read_lines(tmp.path()) {
        for key in line.as_object().unwrap().keys() {
            assert!(allowed.contains(&key.as_str()), "unexpected key {key}");
        }
    }
}
