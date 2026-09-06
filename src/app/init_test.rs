//! Tests for `App::new` construction (see `init.rs`).
//!
//! These cover the directory-walk and config-driven visibility behaviour the
//! constructor is responsible for. Git-status seeding is exercised separately
//! in the git-mode tests in `mod_test.rs`.

use std::fs;
use std::path::PathBuf;
use std::sync::atomic::{AtomicUsize, Ordering};

use super::*;
use crate::app::DiffMode;
use crate::config::{Config, ContentConfig};

fn temp_dir() -> PathBuf {
    static COUNTER: AtomicUsize = AtomicUsize::new(0);
    let n = COUNTER.fetch_add(1, Ordering::Relaxed);
    let dir = std::env::temp_dir().join(format!("tv_init_test_{}_{n}", std::process::id()));
    fs::create_dir_all(&dir).unwrap();
    dir.canonicalize().unwrap()
}

fn new_app(root: &std::path::Path, cfg: Config) -> App {
    App::new(root.to_path_buf(), cfg, None, None).unwrap()
}

#[test]
fn app_new_builds_visible_root_tree() {
    let root = temp_dir();
    fs::create_dir(root.join("sub")).unwrap();
    fs::write(root.join("a.txt"), "one\n").unwrap();
    fs::write(root.join("b.txt"), "two\n").unwrap();

    let app = new_app(&root, Config::default());

    assert_eq!(app.tree_selected, 0);
    let names: Vec<&str> = app.nodes.iter().map(|n| n.name.as_str()).collect();
    assert!(names.contains(&"a.txt"), "got {names:?}");
    assert!(names.contains(&"b.txt"), "got {names:?}");
    assert!(names.contains(&"sub"), "got {names:?}");
    fs::remove_dir_all(&root).ok();
}

#[test]
fn app_new_starts_with_no_tree_guide_cache() {
    // The indent-guide mask cache is keyed by tree_revision and must start
    // empty so the first render always computes it fresh.
    let root = temp_dir();
    let app = new_app(&root, Config::default());
    assert!(app.tree_guide_cache.is_none());
    fs::remove_dir_all(&root).ok();
}

#[test]
fn app_new_starts_with_no_plugin_contributions() {
    // A freshly constructed App has produced no plugin output yet, so the
    // per-plugin contribution tracker must be empty.
    let root = temp_dir();
    fs::write(root.join("a.txt"), "one\n").unwrap();

    let app = new_app(&root, Config::default());

    assert!(
        app.plugin_contributions.is_empty(),
        "new App must have no plugin contributions"
    );
    fs::remove_dir_all(&root).ok();
}

#[test]
fn app_new_plugin_open_guard_defaults_false() {
    // The re-entrancy guard that suppresses `on_file_open` re-emission for
    // plugin-originated opens must start cleared.
    let root = temp_dir();
    let app = new_app(&root, Config::default());
    assert!(!app.plugin_is_opening_file);
    fs::remove_dir_all(&root).ok();
}

#[test]
fn app_new_starts_with_empty_plugin_content() {
    // Fresh App must have no plugin-provided content cached, neither the styled
    // spans nor the parallel plain-text store.
    let root = temp_dir();
    let app = new_app(&root, Config::default());
    assert!(
        app.plugin_content.is_empty(),
        "plugin_content must start empty"
    );
    assert!(
        app.plugin_content_text.is_empty(),
        "plugin_content_text must start empty"
    );
    assert!(
        app.plugin_status_facts.is_empty(),
        "plugin_status_facts must start empty"
    );
    fs::remove_dir_all(&root).ok();
}

#[test]
fn app_new_hides_dotfiles_by_default() {
    let root = temp_dir();
    fs::write(root.join("visible.txt"), "x\n").unwrap();
    fs::write(root.join(".hidden"), "y\n").unwrap();

    let app = new_app(&root, Config::default());

    let names: Vec<&str> = app.nodes.iter().map(|n| n.name.as_str()).collect();
    assert!(names.contains(&"visible.txt"), "got {names:?}");
    assert!(
        !names.contains(&".hidden"),
        "dotfile must be hidden; got {names:?}"
    );
    fs::remove_dir_all(&root).ok();
}

#[test]
fn app_new_show_hidden_includes_dotfiles() {
    let root = temp_dir();
    fs::write(root.join(".hidden"), "y\n").unwrap();

    let cfg = Config {
        tree: crate::config::TreeConfig {
            show_hidden: true,
            ..Default::default()
        },
        ..Config::default()
    };
    let app = new_app(&root, cfg);

    let names: Vec<&str> = app.nodes.iter().map(|n| n.name.as_str()).collect();
    assert!(names.contains(&".hidden"), "got {names:?}");
    fs::remove_dir_all(&root).ok();
}

#[test]
fn app_new_registers_syntax_plugins_in_manager_for_palette() {
    // init.rs hands *all* plugin entries (including syntax-kind) to the
    // PluginManager so they surface in the plugin palette; the bundled
    // toml syntax plugin is seeded into the config by default.
    let root = temp_dir();
    let app = new_app(&root, Config::default());
    let entries = app.plugin_manager.plugin_entries();
    assert!(
        entries
            .iter()
            .any(|(_, _, kind, _)| *kind == crate::plugin::PluginKind::Syntax),
        "a syntax plugin must be registered in the manager so it appears in the \
         plugin palette; got {entries:?}"
    );
    fs::remove_dir_all(&root).ok();
}

#[test]
#[cfg(unix)]
fn app_new_sends_resolved_theme_colors_to_plugins_on_init() {
    // init.rs resolves the configured theme and hands it to
    // `plugin_manager.activate_all` so plugins get real theme colors on
    // `init` instead of just a theme name (they used to have to hardcode a
    // palette per theme name).
    use std::io::Write as _;
    use std::os::unix::fs::PermissionsExt;
    use std::time::{Duration, Instant};

    let root = temp_dir();
    let plugin_dir = std::env::temp_dir().join(format!(
        "tv_init_colors_plugin_{}_{}",
        std::process::id(),
        root.file_name().unwrap().to_string_lossy()
    ));
    fs::create_dir_all(&plugin_dir).unwrap();
    let out = plugin_dir.join("recv.txt");
    let script = plugin_dir.join("rec.sh");
    let mut f = fs::File::create(&script).unwrap();
    write!(f, "#!/bin/sh\ncat > \"{}\"\n", out.display()).unwrap();
    drop(f);
    fs::set_permissions(&script, fs::Permissions::from_mode(0o755)).unwrap();

    let mut cfg = Config::default();
    cfg.theme.name = Some("monokai".into());
    cfg.plugins.insert(
        "rec".to_string(),
        crate::plugin::PluginEntry {
            path: script.clone(),
            enabled: true,
            ..Default::default()
        },
    );

    let mut app = new_app(&root, cfg);
    app.plugin_manager.deactivate_all();

    let deadline = Instant::now() + Duration::from_secs(3);
    let contents = loop {
        if let Ok(s) = fs::read_to_string(&out) {
            if !s.is_empty() {
                break s;
            }
        }
        assert!(Instant::now() < deadline, "plugin never received init");
        std::thread::sleep(Duration::from_millis(25));
    };
    let init_line = contents
        .lines()
        .find(|l| l.contains(r#""event":"init""#))
        .expect("init event must be sent");
    let monokai = crate::theme::Theme::load("monokai").expect("monokai theme must load");
    let expected = format!(
        r#""heading1":"{}""#,
        crate::theme::color_to_hex(monokai.heading1)
    );
    assert!(
        init_line.contains(&expected),
        "init colors must carry the configured theme's actual heading1 hex, got: {init_line}"
    );
    fs::remove_dir_all(&root).ok();
    fs::remove_dir_all(&plugin_dir).ok();
}

#[test]
fn app_new_bundled_plugins_appear_in_config_plugins_map() {
    // Regression: bundled/manifest plugins were seeded into `cfg.plugins` only
    // *after* `saved_config = cfg.clone()`, so `self.config.plugins` was empty
    // and `toggle_plugin_picker_selection` could never persist the enabled flag.
    let root = temp_dir();
    let app = new_app(&root, Config::default());
    assert!(
        !app.config.plugins.is_empty(),
        "bundled plugins must appear in config.plugins; got empty map"
    );
    // At least one bundled entry should be present (e.g. the markdown plugin).
    let bundled: Vec<String> = crate::plugin::bundled_plugin_entries()
        .into_iter()
        .map(|(n, _)| n)
        .collect();
    for name in &bundled {
        assert!(
            app.config.plugins.contains_key(name),
            "bundled plugin {name} must appear in config.plugins"
        );
    }
    fs::remove_dir_all(&root).ok();
}

#[test]
fn app_new_bundled_plugin_toggle_flips_enabled_flag() {
    // Toggling a bundled plugin's enabled flag via config.plugins.get_mut
    // must succeed because the entry is present in self.config.plugins.
    let root = temp_dir();
    let mut app = new_app(&root, Config::default());
    let bundled: Vec<String> = crate::plugin::bundled_plugin_entries()
        .into_iter()
        .map(|(n, _)| n)
        .collect();
    let name = bundled.first().expect("at least one bundled plugin");
    let orig = app
        .config
        .plugins
        .get(name)
        .map(|e| e.enabled)
        .unwrap_or(false);
    if let Some(entry) = app.config.plugins.get_mut(name) {
        entry.enabled = !orig;
    }
    let flipped = app
        .config
        .plugins
        .get(name)
        .map(|e| e.enabled)
        .unwrap_or(orig);
    assert_ne!(
        orig, flipped,
        "toggling bundled plugin {name}: enabled should have flipped from {orig}"
    );
    fs::remove_dir_all(&root).ok();
}

#[test]
fn app_new_preserves_root_path() {
    let root = temp_dir();
    let app = new_app(&root, Config::default());
    assert_eq!(app.root, root);
    fs::remove_dir_all(&root).ok();
}

#[test]
fn app_new_starts_in_normal_mode() {
    let root = temp_dir();
    fs::write(root.join("f.txt"), "x\n").unwrap();
    let app = new_app(&root, Config::default());
    assert!(!app.git_mode, "App::new must always start in normal mode");
    fs::remove_dir_all(&root).ok();
}

#[test]
fn app_new_starts_with_no_compare_mode() {
    let root = temp_dir();
    fs::write(root.join("f.txt"), "x\n").unwrap();
    let app = new_app(&root, Config::default());
    assert!(app.compare_base.is_none());
    assert!(app.revision_picker.is_none());
    assert!(app.bug_report.is_none());
    fs::remove_dir_all(&root).ok();
}

#[test]
fn app_new_session_git_mode_ignored() {
    let root = temp_dir();
    fs::write(root.join("f.txt"), "x\n").unwrap();
    // Manually write a session file with old-format git_mode: true.
    let key = root.to_string_lossy();
    let old = format!(
        r#"{{"version":1,"sessions":{{"{}":{{"expanded":[],"current_file":null,"content_scroll":0,"active_line":0,"git_mode":true}}}}}}"#,
        key
    );
    if let Some(p) = crate::session::sessions_path() {
        if let Some(parent) = p.parent() {
            std::fs::create_dir_all(parent).unwrap();
        }
        std::fs::write(&p, &old).unwrap();
    }
    let app = new_app(&root, Config::default());
    assert!(
        !app.git_mode,
        "must start in normal mode even when session has git_mode"
    );
    fs::remove_dir_all(&root).ok();
}

#[test]
fn app_new_viewing_revision_starts_none() {
    let root = temp_dir();
    let app = new_app(&root, Config::default());
    assert!(
        app.viewing_revision.is_none(),
        "App::new must initialize viewing_revision to None"
    );
    fs::remove_dir_all(&root).ok();
}

#[test]
fn app_new_git_seq_starts_zero() {
    let root = temp_dir();
    let app = new_app(&root, Config::default());
    assert_eq!(app.git_seq, 0, "git_seq must be zero on construction");
    fs::remove_dir_all(&root).ok();
}

#[test]
fn app_new_git_show_flags_reflect_config() {
    let root = temp_dir();
    fs::write(root.join("f.txt"), "x\n").unwrap();
    let cfg = Config {
        git: crate::config::GitConfig {
            show_untracked: false,
            show_ignored: true,
            ..Default::default()
        },
        ..Config::default()
    };
    let app = new_app(&root, cfg);
    assert!(
        !app.git_show_untracked,
        "git_show_untracked must come from config"
    );
    assert!(
        app.git_show_ignored,
        "git_show_ignored must come from config"
    );
    fs::remove_dir_all(&root).ok();
}

#[test]
fn app_new_last_breadcrumb_click_is_none() {
    let root = temp_dir();
    let app = new_app(&root, Config::default());
    assert!(
        app.last_breadcrumb_click.is_none(),
        "last_breadcrumb_click must be None on construction"
    );
    fs::remove_dir_all(&root).ok();
}

#[test]
fn app_new_highlight_cache_starts_empty() {
    let root = temp_dir();
    fs::write(root.join("a.txt"), "x\n").unwrap();
    let app = new_app(&root, Config::default());
    assert!(
        app.content_highlight_cache.borrow().is_none(),
        "fresh App must have no cached highlights"
    );
    fs::remove_dir_all(&root).ok();
}

#[test]
fn app_new_diff_mode_defaults_to_all() {
    let root = temp_dir();
    fs::write(root.join("f.txt"), "x\n").unwrap();
    let app = new_app(&root, Config::default());
    assert_eq!(app.diff_mode, DiffMode::All);
    fs::remove_dir_all(&root).ok();
}

#[test]
fn app_new_diff_mode_honours_config_staged() {
    let root = temp_dir();
    fs::write(root.join("f.txt"), "x\n").unwrap();
    let cfg = Config {
        git: crate::config::GitConfig {
            diff: crate::config::GitDiffConfig {
                mode: crate::app::DiffMode::Staged,
                ..Default::default()
            },
            ..Default::default()
        },
        ..Config::default()
    };
    let app = new_app(&root, cfg);
    assert_eq!(app.diff_mode, DiffMode::Staged);
    fs::remove_dir_all(&root).ok();
}

#[test]
fn app_new_diff_mode_honours_config_unstaged() {
    let root = temp_dir();
    fs::write(root.join("f.txt"), "x\n").unwrap();
    let cfg = Config {
        git: crate::config::GitConfig {
            diff: crate::config::GitDiffConfig {
                mode: crate::app::DiffMode::Unstaged,
                ..Default::default()
            },
            ..Default::default()
        },
        ..Config::default()
    };
    let app = new_app(&root, cfg);
    assert_eq!(app.diff_mode, DiffMode::Unstaged);
    fs::remove_dir_all(&root).ok();
}

#[test]
fn app_new_diff_mode_invalid_falls_back_to_all() {
    let root = temp_dir();
    fs::write(root.join("f.txt"), "x\n").unwrap();
    let cfg = Config {
        legacy_diff_mode: Some("invalid".to_string()),
        ..Config::default()
    };
    let app = new_app(&root, cfg);
    assert_eq!(app.diff_mode, DiffMode::All);
    fs::remove_dir_all(&root).ok();
}

#[test]
fn app_new_command_usage_starts_empty() {
    let _lock = crate::session::STATE_DIR_ENV_LOCK
        .lock()
        .unwrap_or_else(|e| e.into_inner());
    let root = temp_dir();
    // Point at a fresh temp dir so no on-disk usage data is loaded.
    let state_dir = temp_dir();
    std::env::set_var("MANTIS_STATE_DIR", &state_dir);
    let app = new_app(&root, Config::default());
    std::env::remove_var("MANTIS_STATE_DIR");
    assert!(
        app.command_usage.last_used().is_none(),
        "command_usage.last_used must be None when state dir is empty"
    );
    assert!(
        app.command_usage.top_used(1).is_empty(),
        "command_usage must have no recorded commands when state dir is empty"
    );
    fs::remove_dir_all(&root).ok();
    fs::remove_dir_all(&state_dir).ok();
}

#[test]
fn blame_area_initialises_to_default() {
    let root = temp_dir();
    let app = new_app(&root, Config::default());
    assert_eq!(
        app.blame_area,
        ratatui::layout::Rect::default(),
        "blame_area must be default (inactive) until first render"
    );
    assert!(!app.blame_before_commit);
    fs::remove_dir_all(&root).ok();
}

#[test]
fn help_scroll_initialises_to_zero() {
    let root = temp_dir();
    let app = new_app(&root, Config::default());
    assert_eq!(
        app.help_scroll.scroll, 0,
        "help_scroll must start at zero so the help popup opens unscrolled"
    );
    assert_eq!(
        app.help_tab, 0,
        "help_tab must start at zero (Getting started tab)"
    );
    fs::remove_dir_all(&root).ok();
}

#[test]
fn plugin_content_active_path_initialises_to_none() {
    let root = temp_dir();
    let app = new_app(&root, Config::default());
    assert!(
        app.plugin_content_active_path.is_none(),
        "plugin_content_active_path must be None at construction so the first set_content is treated as first-render"
    );
    fs::remove_dir_all(&root).ok();
}

#[test]
fn app_new_tree_revision_starts_at_zero() {
    let root = temp_dir();
    let app = new_app(&root, Config::default());
    assert_eq!(
        app.tree_revision, 0,
        "tree_revision must be 0 at construction"
    );
    fs::remove_dir_all(&root).ok();
}

#[test]
fn app_new_tree_visible_indices_starts_none() {
    let root = temp_dir();
    let app = new_app(&root, Config::default());
    assert!(
        app.tree_visible_indices.is_none(),
        "tree_visible_indices must be None when no filter is active"
    );
    fs::remove_dir_all(&root).ok();
}

#[test]
fn app_new_cursor_positions_starts_empty() {
    let root = temp_dir();
    let app = new_app(&root, Config::default());
    assert!(
        app.cursor_positions.is_empty(),
        "cursor_positions must start empty"
    );
    fs::remove_dir_all(&root).ok();
}

#[test]
fn app_new_clipboard_capture_starts_empty() {
    // The test-only clipboard seam (see `copy_to_clipboard` in mod.rs) must
    // start empty so the first capture assertion in a test reflects only
    // that test's own clipboard writes.
    let root = temp_dir();
    let app = new_app(&root, Config::default());
    assert!(
        app.clipboard_capture.is_empty(),
        "clipboard_capture must start empty on construction"
    );
    fs::remove_dir_all(&root).ok();
}

#[test]
fn app_new_secret_masking_state_starts_clear() {
    let root = temp_dir();
    let app = new_app(&root, Config::default());
    assert!(app.secret_original.is_empty());
    assert!(!app.secret_masked);
    assert!(!app.secret_revealed);
    fs::remove_dir_all(&root).ok();
}

#[test]
fn app_new_ignore_gitignore_includes_ignored_in_status_map() {
    let root = temp_dir();
    let git = |args: &[&str]| {
        let status = std::process::Command::new("git")
            .arg("-C")
            .arg(&root)
            .args(["-c", "user.email=t@e.x", "-c", "user.name=T"])
            .args(args)
            .status()
            .unwrap();
        assert!(status.success(), "git {:?} failed", args);
    };
    git(&["init", "-q"]);
    fs::write(root.join("tracked.txt"), "hello\n").unwrap();
    fs::write(root.join(".gitignore"), "*.log\n").unwrap();
    git(&["add", "."]);
    git(&["commit", "-q", "-m", "init"]);
    fs::write(root.join("build.log"), "log\n").unwrap();

    let cfg = Config {
        git: crate::config::GitConfig {
            ignore_gitignore: true,
            // show_ignored defaults to false — must be overridden by ignore_gitignore.
            ..Default::default()
        },
        ..Config::default()
    };
    let app = new_app(&root, cfg);
    let root_canon = root.canonicalize().unwrap();
    let ignored = root_canon.join("build.log");
    assert_eq!(
        app.git_status_map.get(&ignored),
        Some(&crate::git::GitStatus::Ignored),
        "ignore_gitignore=true must seed ignored entries even when show_ignored=false"
    );
    fs::remove_dir_all(&root).ok();
}
#[test]
fn app_new_respects_prettify_size_limit_for_json_file() {
    let root = temp_dir();
    let json = root.join("data.json");
    let data = serde_json::json!({
        "items": (0..20).map(|i| format!("val_{}", i)).collect::<Vec<_>>()
    });
    fs::write(&json, serde_json::to_string(&data).unwrap()).unwrap();

    // Config with a very small limit so the test JSON exceeds it.
    let cfg = Config {
        content: ContentConfig {
            prettify_size_limit: 50,
            ..Default::default()
        },
        ..Config::default()
    };
    let app = new_app(&root, cfg);
    // The constructor opens the first selected file (data.json).
    // With the low limit it must be shown as raw content.
    assert!(
        !app.show_pretty_json,
        "JSON exceeding prettify_size_limit must not be pretty-printed"
    );
    fs::remove_dir_all(&root).ok();
}

#[test]
fn app_new_starts_with_no_update_notice_regardless_of_config() {
    use crate::config::UpdatesConfig;

    let root = temp_dir();
    for check in [true, false] {
        let cfg = Config {
            updates: UpdatesConfig { check },
            ..Config::default()
        };
        let app = new_app(&root, cfg);
        // check_for_updates() is a no-op in test builds, so App::new must
        // never surface a stale/fabricated notice regardless of the config.
        assert!(app.new_version_available.is_none());
        assert!(app.update_rx.is_none());
    }
    fs::remove_dir_all(&root).ok();
}

#[test]
fn telemetry_disabled_by_default() {
    let dir = temp_dir();
    let app = new_app(&dir, Config::default());
    assert!(!app.telemetry.is_enabled());
}

#[test]
fn telemetry_enabled_when_configured() {
    let _guard = crate::session::STATE_DIR_ENV_LOCK
        .lock()
        .unwrap_or_else(|e| e.into_inner());
    let state = tempfile::tempdir().unwrap();
    std::env::set_var("MANTIS_STATE_DIR", state.path());
    let dir = temp_dir();
    let cfg = Config {
        telemetry: crate::config::TelemetryConfig {
            enabled: true,
            notice_shown: false,
        },
        ..Config::default()
    };
    let app = new_app(&dir, cfg);
    assert!(app.telemetry.is_enabled());
    drop(app);
    let telemetry_dir = state.path().join("telemetry");
    assert!(std::fs::read_dir(&telemetry_dir)
        .unwrap()
        .flatten()
        .any(|e| e
            .file_name()
            .to_str()
            .is_some_and(|n| n.starts_with("events-") && n.ends_with(".jsonl"))));
    std::env::remove_var("MANTIS_STATE_DIR");
}

#[test]
fn app_new_initializes_initial_root() {
    let root = temp_dir();
    let app = new_app(&root, Config::default());
    assert_eq!(app.initial_root, root);
    assert!(app.bookmark_paths.is_empty());
    fs::remove_dir_all(&root).ok();
}

#[test]
fn app_new_ignores_stale_initial_root_not_an_ancestor_of_launch_root() {
    // Simulates a stale session file whose `initial_root` points outside the
    // current launch root — e.g. because the same directory was previously
    // reached by descending from an unrelated ancestor. The restored
    // `initial_root` must never be trusted unless it actually contains the
    // directory mantis was just launched with, otherwise the up-dir clamp
    // could let navigation escape past the real launch root.
    let _lock = crate::session::STATE_DIR_ENV_LOCK
        .lock()
        .unwrap_or_else(|e| e.into_inner());
    let root = temp_dir();
    let unrelated = temp_dir();
    let state_dir = temp_dir();
    std::env::set_var("MANTIS_STATE_DIR", &state_dir);

    crate::session::save(
        &root,
        &crate::session::SessionState {
            expanded: Vec::new(),
            bookmarks: Vec::new(),
            current_file: None,
            content_scroll: 0,
            active_line: 0,
            initial_root: Some(unrelated.clone()),
        },
    );

    let app = new_app(&root, Config::default());
    std::env::remove_var("MANTIS_STATE_DIR");

    assert_eq!(
        app.initial_root, root,
        "initial_root from an unrelated stale session must be ignored"
    );
    fs::remove_dir_all(&root).ok();
    fs::remove_dir_all(&unrelated).ok();
    fs::remove_dir_all(&state_dir).ok();
}

#[test]
fn app_new_ignores_stale_initial_root_that_is_ancestor_of_launch_root() {
    // A stale `initial_root` that is an ancestor of (but not equal to) the
    // current launch root must also be rejected: `starts_with` alone would
    // wrongly accept it, letting the up-dir clamp restore a wider stale
    // boundary than the directory mantis was actually launched with.
    let _lock = crate::session::STATE_DIR_ENV_LOCK
        .lock()
        .unwrap_or_else(|e| e.into_inner());
    let ancestor = temp_dir();
    let root = ancestor.join("nested");
    fs::create_dir_all(&root).unwrap();
    let state_dir = temp_dir();
    std::env::set_var("MANTIS_STATE_DIR", &state_dir);

    crate::session::save(
        &root,
        &crate::session::SessionState {
            expanded: Vec::new(),
            bookmarks: Vec::new(),
            current_file: None,
            content_scroll: 0,
            active_line: 0,
            initial_root: Some(ancestor.clone()),
        },
    );

    let app = new_app(&root, Config::default());
    std::env::remove_var("MANTIS_STATE_DIR");

    assert_eq!(
        app.initial_root, root,
        "initial_root from an ancestor-only stale session must be ignored"
    );
    fs::remove_dir_all(&ancestor).ok();
    fs::remove_dir_all(&state_dir).ok();
}

#[test]
fn init_telemetry_check() {
    let root = temp_dir();
    let app = new_app(&root, Config::default());
    assert!(!app.telemetry.is_enabled());
    fs::remove_dir_all(&root).ok();
}

// -- welcome overlay ---------------------------------------------------------

#[test]
fn app_new_starts_with_welcome_disabled() {
    // App::new() does not inspect the welcome flag — that is done by the
    // production `run_app` wrapper. The struct field defaults to false.
    let _lock = crate::session::STATE_DIR_ENV_LOCK
        .lock()
        .unwrap_or_else(|e| e.into_inner());
    let root = temp_dir();
    let state_dir = temp_dir();
    std::env::set_var("MANTIS_STATE_DIR", &state_dir);

    let app = new_app(&root, Config::default());
    std::env::remove_var("MANTIS_STATE_DIR");

    assert!(!app.show_welcome, "App::new must set show_welcome = false");
    fs::remove_dir_all(&root).ok();
    fs::remove_dir_all(&state_dir).ok();
}

#[test]
fn app_new_starts_without_repo_log_overlay() {
    let root = temp_dir();
    let app = new_app(&root, Config::default());
    assert!(app.repo_log.is_none());
    assert_eq!(app.repo_log_offset, 0);
    fs::remove_dir_all(&root).ok();
}

#[test]
fn app_new_starts_without_worktree_picker() {
    let root = temp_dir();
    let app = new_app(&root, Config::default());
    assert!(app.worktree_picker.is_none());
    fs::remove_dir_all(&root).ok();
}

// -- file_at_revision / viewing_revision_hash ----------------------------------

#[test]
fn app_new_starts_with_file_at_revision_none() {
    let root = temp_dir();
    let app = new_app(&root, Config::default());
    assert!(
        app.file_at_revision.is_none(),
        "App::new must set file_at_revision = None"
    );
    fs::remove_dir_all(&root).ok();
}

#[test]
fn app_new_starts_with_viewing_revision_hash_none() {
    let root = temp_dir();
    let app = new_app(&root, Config::default());
    assert!(
        app.viewing_revision_hash.is_none(),
        "App::new must set viewing_revision_hash = None"
    );
    fs::remove_dir_all(&root).ok();
}

#[test]
fn app_new_starts_with_show_raw_markdown_false() {
    let root = temp_dir();
    let app = new_app(&root, Config::default());
    assert!(
        !app.show_raw_markdown,
        "App::new must set show_raw_markdown = false"
    );
    fs::remove_dir_all(&root).ok();
}
// touched for log follow mode

#[test]
fn app_new_starts_with_no_context_menu() {
    let root = temp_dir();
    let app = new_app(&root, Config::default());
    assert!(
        app.context_menu.is_none(),
        "App::new must not open a context menu"
    );
    assert_eq!(
        app.context_menu_area,
        ratatui::layout::Rect::default(),
        "App::new must start with a default context-menu hit area"
    );
    fs::remove_dir_all(&root).ok();
}

#[test]
fn app_new_starts_without_jsonl_state() {
    let root = temp_dir();
    let app = new_app(&root, Config::default());
    assert!(!app.is_jsonl);
    assert!(app.jsonl_source.is_empty());
    fs::remove_dir_all(&root).ok();
}
#[test]
fn app_new_starts_with_follow_mode_disabled() {
    let root = temp_dir();
    let app = new_app(&root, Config::default());
    assert!(!app.follow_mode);
    assert!(!app.follow_pinned);
    fs::remove_dir_all(&root).ok();
}

#[test]
fn app_new_starts_without_csv_state() {
    let root = temp_dir();
    let app = new_app(&root, Config::default());
    assert!(!app.is_csv);
    assert!(!app.show_csv_table);
    assert!(app.csv_table_text.is_empty());
    assert!(app.csv_table_lines.is_empty());
    fs::remove_dir_all(&root).ok();
}

#[test]
fn app_new_starts_without_inline_image_state() {
    let root = temp_dir();
    let app = new_app(&root, Config::default());
    assert!(app.content_image.is_none());
    assert_eq!(app.image_area, ratatui::layout::Rect::default());
    fs::remove_dir_all(&root).ok();
}

#[test]
fn app_new_installs_bundled_plugins_on_startup() {
    let _guard = crate::plugin::ENV_LOCK
        .lock()
        .unwrap_or_else(|e| e.into_inner());
    let tmp = std::env::temp_dir().join(format!(
        "mantis_app_new_plugin_install_{}_{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_nanos())
            .unwrap_or(0)
    ));
    std::fs::create_dir_all(&tmp).unwrap();
    let var = if cfg!(windows) {
        "APPDATA"
    } else {
        "XDG_CONFIG_HOME"
    };
    let old = std::env::var_os(var);
    unsafe { std::env::set_var(var, &tmp) };

    let root = temp_dir();
    let mut app = new_app(&root, Config::default());
    app.plugin_manager.deactivate_all();

    let plugin_dir = tmp.join("mantis").join("plugins");
    assert!(
        plugin_dir.exists(),
        "App::new must ensure default plugin dir is created and populated"
    );

    unsafe {
        match old {
            Some(v) => std::env::set_var(var, v),
            None => std::env::remove_var(var),
        }
    }
    fs::remove_dir_all(&root).ok();
    std::fs::remove_dir_all(&tmp).ok();
}
