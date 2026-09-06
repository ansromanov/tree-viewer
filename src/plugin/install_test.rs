use std::path::PathBuf;
use std::sync::atomic::{AtomicUsize, Ordering};

use super::*;

use crate::plugin::install::remove_retired_bundled_plugins;

fn fresh_plugin_dir() -> PathBuf {
    static COUNTER: AtomicUsize = AtomicUsize::new(0);
    let n = COUNTER.fetch_add(1, Ordering::Relaxed);
    std::env::temp_dir().join(format!("mantis_plugin_install_{}_{n}", std::process::id()))
}

/// The env var `default_plugin_dir` consults for the config root: `APPDATA`
/// on Windows, `XDG_CONFIG_HOME` elsewhere. Tests must override this one
/// rather than hardcoding `XDG_CONFIG_HOME`, which Windows ignores.
fn config_home_var() -> &'static str {
    if cfg!(windows) {
        "APPDATA"
    } else {
        "XDG_CONFIG_HOME"
    }
}

#[test]
#[cfg(not(windows))]
fn default_plugin_dir_ends_with_suffix() {
    let dir = default_plugin_dir();
    let components: Vec<_> = dir.components().collect();
    let last_two: Vec<_> = components.iter().rev().take(2).collect();
    assert_eq!(
        last_two[0],
        &std::path::Component::Normal("plugins".as_ref())
    );
    assert_eq!(
        last_two[1],
        &std::path::Component::Normal("mantis".as_ref())
    );
}

#[test]
#[cfg(not(windows))]
fn default_plugin_dir_respects_xdg() {
    let _guard = ENV_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let old = std::env::var_os("XDG_CONFIG_HOME");
    unsafe { std::env::set_var("XDG_CONFIG_HOME", "/tmp/custom_cfg") };
    let dir = default_plugin_dir();
    unsafe {
        match old {
            Some(v) => std::env::set_var("XDG_CONFIG_HOME", v),
            None => std::env::remove_var("XDG_CONFIG_HOME"),
        }
    }
    assert!(dir.starts_with("/tmp/custom_cfg/mantis/plugins"));
}

#[test]
fn bundled_plugin_entries_all_enabled_by_default() {
    let entries = bundled_plugin_entries();
    assert!(!entries.is_empty(), "must have at least one bundled plugin");
    let names: Vec<&str> = entries.iter().map(|(n, _)| n.as_str()).collect();
    assert!(
        names.contains(&"markdown"),
        "markdown plugin must be listed"
    );
    assert!(names.contains(&"iconize"), "iconize plugin must be listed");
    assert!(names.contains(&"rust"), "rust plugin must be listed");
    assert!(names.contains(&"go"), "go plugin must be listed");
    assert!(
        names.contains(&"terraform"),
        "terraform language provider plugin must be listed"
    );
    assert!(names.contains(&"toml"), "toml syntax plugin must be listed");
    assert!(
        names.contains(&"typescript"),
        "typescript syntax plugin must be listed"
    );
    assert!(
        names.contains(&"dockerfile"),
        "dockerfile syntax plugin must be listed"
    );
    assert!(
        names.contains(&"nginx"),
        "nginx syntax plugin must be listed"
    );
    assert!(
        names.contains(&"justfile"),
        "justfile syntax plugin must be listed"
    );
    assert!(names.contains(&"python"), "python plugin must be listed");
    assert!(names.contains(&"json"), "json plugin must be listed");
    assert!(names.contains(&"sh"), "sh plugin must be listed");
    assert!(names.contains(&"yaml"), "yaml plugin must be listed");
    assert!(names.contains(&"k8s"), "k8s plugin must be listed");
    for (name, entry) in &entries {
        assert!(
            entry.enabled,
            "bundled plugin {name} must default to enabled=true"
        );
    }
    // Process entries
    for (name, entry) in &entries {
        if name == "toml"
            || name == "typescript"
            || name == "dockerfile"
            || name == "nginx"
            || name == "justfile"
        {
            assert_eq!(
                entry.kind,
                PluginKind::Syntax,
                "{name} must be a syntax plugin"
            );
            assert!(
                entry.syntax_file.is_some(),
                "{name} must have syntax_file set"
            );
        } else {
            assert_eq!(
                entry.kind,
                PluginKind::Process,
                "all other bundled entries must be process plugins"
            );
        }
    }
}

#[test]
fn bundled_plugin_entries_use_relative_paths() {
    // Regression: bundled entries land in `app.config.plugins` and get
    // serialised into the user's `tv.toml` on the first plugin toggle. Absolute
    // paths would pin a machine-specific home directory into a portable config,
    // so every bundled `path` must be relative (resolved against the plugin dir
    // at spawn time).
    for (name, entry) in bundled_plugin_entries() {
        assert!(
            entry.path.is_relative(),
            "bundled plugin {name} path must be relative, got {:?}",
            entry.path
        );
    }
}

#[test]
fn bundled_plugin_entries_have_empty_events() {
    // Bundled entries declare no event subscription, so they receive all
    // events (empty = all, backward compat). Manifest-discovered plugins are
    // the ones that opt into a subset.
    for (name, entry) in bundled_plugin_entries() {
        assert!(
            entry.events.is_empty(),
            "bundled plugin {name} must not pin an events subscription"
        );
    }
}

#[test]
fn bundled_plugin_entries_no_duplicates() {
    let entries = bundled_plugin_entries();
    let mut seen = std::collections::HashSet::new();
    for (name, _) in &entries {
        assert!(seen.insert(name.as_str()), "duplicate entry: {name}");
    }
}

#[test]
fn install_bundled_plugins_creates_iconize_binary() {
    let _guard = ENV_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let tmp = fresh_plugin_dir();
    std::fs::create_dir_all(&tmp).unwrap();
    let old = std::env::var_os(config_home_var());
    unsafe { std::env::set_var(config_home_var(), &tmp) };

    install_bundled_plugins();

    unsafe {
        match old {
            Some(v) => std::env::set_var(config_home_var(), v),
            None => std::env::remove_var(config_home_var()),
        }
    }

    let plugins_dir = tmp.join("mantis").join("plugins");
    assert!(plugins_dir.is_dir(), "plugins directory should be created");
    assert!(
        plugins_dir.join("syntaxes").is_dir(),
        "syntaxes subdirectory should be created"
    );
    assert!(
        plugins_dir
            .join("syntaxes")
            .join("terraform.sublime-syntax")
            .exists(),
        "terraform.sublime-syntax must be installed"
    );
    assert!(
        plugins_dir
            .join("syntaxes")
            .join("toml.sublime-syntax")
            .exists(),
        "toml.sublime-syntax must be installed"
    );
    assert!(
        plugins_dir
            .join("syntaxes")
            .join("typescript.sublime-syntax")
            .exists(),
        "typescript.sublime-syntax must be installed"
    );
    assert!(
        plugins_dir
            .join("syntaxes")
            .join("dockerfile.sublime-syntax")
            .exists(),
        "dockerfile.sublime-syntax must be installed"
    );
    assert!(
        plugins_dir
            .join("syntaxes")
            .join("nginx.sublime-syntax")
            .exists(),
        "nginx.sublime-syntax must be installed"
    );
    assert!(
        plugins_dir
            .join("syntaxes")
            .join("justfile.sublime-syntax")
            .exists(),
        "justfile.sublime-syntax must be installed"
    );
    let iconize_name = if cfg!(windows) {
        "iconize.exe"
    } else {
        "iconize"
    };
    let markdown_name = if cfg!(windows) {
        "markdown.exe"
    } else {
        "markdown"
    };
    assert!(
        plugins_dir.join(iconize_name).exists(),
        "iconize binary must be installed"
    );
    assert!(
        plugins_dir.join(markdown_name).exists(),
        "markdown binary must be installed"
    );
    let terraform_name = if cfg!(windows) {
        "terraform.exe"
    } else {
        "terraform"
    };
    assert!(
        plugins_dir.join(terraform_name).exists(),
        "terraform binary must be installed"
    );
    std::fs::remove_dir_all(&tmp).ok();
}

#[test]
fn install_bundles_terraform_syntax_content() {
    // Guards the `include_str!` source path for the terraform syntax, which
    // moved to plugins/terraform/syntaxes/. A wrong-but-existing path would
    // bundle empty/garbage content while still compiling, so assert the
    // installed file carries the real HCL grammar.
    let _guard = ENV_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let tmp = fresh_plugin_dir();
    std::fs::create_dir_all(&tmp).unwrap();
    let old = std::env::var_os(config_home_var());
    unsafe { std::env::set_var(config_home_var(), &tmp) };

    install_bundled_plugins();

    unsafe {
        match old {
            Some(v) => std::env::set_var(config_home_var(), v),
            None => std::env::remove_var(config_home_var()),
        }
    }

    let syntax = tmp
        .join("mantis")
        .join("plugins")
        .join("syntaxes")
        .join("terraform.sublime-syntax");
    let content = std::fs::read_to_string(&syntax).expect("terraform syntax should be readable");
    assert!(
        content.contains("%YAML"),
        "bundled terraform syntax must be a real .sublime-syntax document"
    );
    assert!(
        content.to_lowercase().contains("terraform"),
        "bundled terraform syntax must contain the HCL/Terraform grammar"
    );
    std::fs::remove_dir_all(&tmp).ok();
}

#[test]
fn install_bundles_new_syntax_content() {
    // Guards the `include_str!` source paths for the new syntax plugins.
    // A wrong-but-existing path would bundle empty/garbage content while
    // still compiling, so assert each installed file carries its real grammar.
    let _guard = ENV_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let tmp = fresh_plugin_dir();
    std::fs::create_dir_all(&tmp).unwrap();
    let old = std::env::var_os(config_home_var());
    unsafe { std::env::set_var(config_home_var(), &tmp) };

    install_bundled_plugins();

    unsafe {
        match old {
            Some(v) => std::env::set_var(config_home_var(), v),
            None => std::env::remove_var(config_home_var()),
        }
    }

    let syntax_dir = tmp.join("mantis").join("plugins").join("syntaxes");

    let toml_content = std::fs::read_to_string(syntax_dir.join("toml.sublime-syntax"))
        .expect("toml syntax should be readable");
    assert!(
        toml_content.contains("%YAML"),
        "bundled toml syntax must be a real .sublime-syntax document"
    );
    assert!(
        toml_content.to_lowercase().contains("toml"),
        "bundled toml syntax must contain the TOML grammar"
    );

    let ts_content = std::fs::read_to_string(syntax_dir.join("typescript.sublime-syntax"))
        .expect("typescript syntax should be readable");
    assert!(
        ts_content.contains("%YAML"),
        "bundled typescript syntax must be a real .sublime-syntax document"
    );
    assert!(
        ts_content.to_lowercase().contains("typescript"),
        "bundled typescript syntax must contain the TypeScript grammar"
    );

    let docker_content = std::fs::read_to_string(syntax_dir.join("dockerfile.sublime-syntax"))
        .expect("dockerfile syntax should be readable");
    assert!(
        docker_content.contains("%YAML"),
        "bundled dockerfile syntax must be a real .sublime-syntax document"
    );
    assert!(
        docker_content.to_lowercase().contains("dockerfile"),
        "bundled dockerfile syntax must contain the Dockerfile grammar"
    );

    let nginx_content = std::fs::read_to_string(syntax_dir.join("nginx.sublime-syntax"))
        .expect("nginx syntax should be readable");
    assert!(
        nginx_content.contains("%YAML"),
        "bundled nginx syntax must be a real .sublime-syntax document"
    );
    assert!(
        nginx_content.to_lowercase().contains("nginx"),
        "bundled nginx syntax must contain the nginx grammar"
    );

    let justfile_content = std::fs::read_to_string(syntax_dir.join("justfile.sublime-syntax"))
        .expect("justfile syntax should be readable");
    assert!(
        justfile_content.contains("%YAML"),
        "bundled justfile syntax must be a real .sublime-syntax document"
    );
    assert!(
        justfile_content.to_lowercase().contains("justfile"),
        "bundled justfile syntax must contain the justfile grammar"
    );

    std::fs::remove_dir_all(&tmp).ok();
}

#[test]
fn remove_retired_plugins_cleans_stale_shell_scripts_and_preserves_user_plugins() {
    let _guard = ENV_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let tmp = fresh_plugin_dir();
    std::fs::create_dir_all(&tmp).unwrap();
    let old = std::env::var_os(config_home_var());
    unsafe { std::env::set_var(config_home_var(), &tmp) };

    let plugins_dir = tmp.join("mantis").join("plugins");
    std::fs::create_dir_all(&plugins_dir).unwrap();

    let retired = retired_bundled_plugins();
    // Seed retired shell scripts
    for name in retired {
        std::fs::write(plugins_dir.join(name), b"# old plugin").unwrap();
    }
    // Seed a user-authored plugin (not in retired list)
    let user_plugin = plugins_dir.join("custom.sh");
    std::fs::write(&user_plugin, b"# user plugin").unwrap();

    // Run cleanup
    remove_retired_bundled_plugins();

    // Retired scripts should be gone
    for name in retired_bundled_plugins() {
        assert!(
            !plugins_dir.join(name).exists(),
            "retired plugin {name} should have been removed"
        );
    }
    // User plugin untouched
    assert!(user_plugin.exists(), "user plugin must not be removed");

    // Idempotent: re-run must be a no-op
    remove_retired_bundled_plugins();
    assert!(user_plugin.exists(), "user plugin must survive re-run");

    unsafe {
        match old {
            Some(v) => std::env::set_var(config_home_var(), v),
            None => std::env::remove_var(config_home_var()),
        }
    }
    std::fs::remove_dir_all(&tmp).ok();
}

#[test]
fn remove_retired_plugins_no_ops_on_empty_dir() {
    let _guard = ENV_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let tmp = fresh_plugin_dir();
    std::fs::create_dir_all(&tmp).unwrap();
    let old = std::env::var_os(config_home_var());
    unsafe { std::env::set_var(config_home_var(), &tmp) };

    let plugins_dir = tmp.join("mantis").join("plugins");
    std::fs::create_dir_all(&plugins_dir).unwrap();

    // No files at all — should not panic
    remove_retired_bundled_plugins();

    unsafe {
        match old {
            Some(v) => std::env::set_var(config_home_var(), v),
            None => std::env::remove_var(config_home_var()),
        }
    }
    std::fs::remove_dir_all(&tmp).ok();
}

#[test]
fn install_bundled_plugins_overwrites_stale_binary() {
    // Regression: a pre-upgrade binary left in place must not survive forever
    // (issue #533) — install_bundled_plugins must overwrite it with the
    // embedded copy when the content differs.
    let _guard = ENV_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let tmp = fresh_plugin_dir();
    std::fs::create_dir_all(&tmp).unwrap();
    let config_home_var = if cfg!(windows) {
        "APPDATA"
    } else {
        "XDG_CONFIG_HOME"
    };
    let old = std::env::var_os(config_home_var);
    unsafe { std::env::set_var(config_home_var, &tmp) };

    let plugins_dir = tmp.join("mantis").join("plugins");
    std::fs::create_dir_all(&plugins_dir).unwrap();
    let markdown_name = if cfg!(windows) {
        "markdown.exe"
    } else {
        "markdown"
    };
    let markdown_path = plugins_dir.join(markdown_name);
    std::fs::write(&markdown_path, b"stale pre-#526 binary").unwrap();

    install_bundled_plugins();

    unsafe {
        match old {
            Some(v) => std::env::set_var(config_home_var, v),
            None => std::env::remove_var(config_home_var),
        }
    }

    let (_, _, embedded) = crate::plugin::install::BUNDLED_PLUGINS
        .iter()
        .find(|(name, _, _)| *name == "markdown")
        .expect("markdown must be a bundled plugin");
    let installed = std::fs::read(&markdown_path).unwrap();
    assert_eq!(
        &installed, embedded,
        "stale binary must be overwritten with the embedded copy"
    );
    std::fs::remove_dir_all(&tmp).ok();
}

#[test]
#[cfg(unix)]
fn install_bundled_plugins_skips_rewrite_when_content_matches() {
    // If the installed binary already matches the embedded one, install must
    // leave its content alone rather than rewriting it, while still repairing
    // the executable bit (e.g. lost via a permissions change or a copied
    // config dir) so the plugin stays spawnable.
    use std::os::unix::fs::PermissionsExt;

    let _guard = ENV_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let tmp = fresh_plugin_dir();
    std::fs::create_dir_all(&tmp).unwrap();
    let old = std::env::var_os(config_home_var());
    unsafe { std::env::set_var(config_home_var(), &tmp) };

    let plugins_dir = tmp.join("mantis").join("plugins");
    std::fs::create_dir_all(&plugins_dir).unwrap();
    let markdown_path = plugins_dir.join("markdown");
    let (_, _, embedded) = crate::plugin::install::BUNDLED_PLUGINS
        .iter()
        .find(|(name, _, _)| *name == "markdown")
        .expect("markdown must be a bundled plugin");
    std::fs::write(&markdown_path, embedded).unwrap();
    std::fs::set_permissions(&markdown_path, std::fs::Permissions::from_mode(0o644)).unwrap();

    install_bundled_plugins();

    unsafe {
        match old {
            Some(v) => std::env::set_var(config_home_var(), v),
            None => std::env::remove_var(config_home_var()),
        }
    }

    let installed = std::fs::read(&markdown_path).unwrap();
    assert_eq!(
        &installed, embedded,
        "up-to-date binary content must not be rewritten"
    );
    let perms = std::fs::metadata(&markdown_path).unwrap().permissions();
    assert_eq!(
        perms.mode() & 0o111,
        0o111,
        "up-to-date binary must still be repaired to executable"
    );
    std::fs::remove_dir_all(&tmp).ok();
}

#[test]
fn install_bundled_plugins_creates_plugin_dir_and_syntaxes() {
    let _guard = ENV_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let tmp = fresh_plugin_dir();
    std::fs::create_dir_all(&tmp).unwrap();
    let old = std::env::var_os(config_home_var());
    unsafe { std::env::set_var(config_home_var(), &tmp) };

    install_bundled_plugins();

    unsafe {
        match old {
            Some(v) => std::env::set_var(config_home_var(), v),
            None => std::env::remove_var(config_home_var()),
        }
    }

    let plugins_dir = tmp.join("mantis").join("plugins");
    assert!(plugins_dir.is_dir(), "plugins directory should be created");
    assert!(
        plugins_dir.join("syntaxes").is_dir(),
        "syntaxes subdirectory should be created"
    );
    assert!(
        plugins_dir
            .join("syntaxes")
            .join("terraform.sublime-syntax")
            .exists(),
        "terraform.sublime-syntax must be installed"
    );
    assert!(
        plugins_dir
            .join("syntaxes")
            .join("toml.sublime-syntax")
            .exists(),
        "toml.sublime-syntax must be installed"
    );
    assert!(
        plugins_dir
            .join("syntaxes")
            .join("typescript.sublime-syntax")
            .exists(),
        "typescript.sublime-syntax must be installed"
    );
    assert!(
        plugins_dir
            .join("syntaxes")
            .join("dockerfile.sublime-syntax")
            .exists(),
        "dockerfile.sublime-syntax must be installed"
    );
    assert!(
        plugins_dir
            .join("syntaxes")
            .join("nginx.sublime-syntax")
            .exists(),
        "nginx.sublime-syntax must be installed"
    );
    assert!(
        plugins_dir
            .join("syntaxes")
            .join("justfile.sublime-syntax")
            .exists(),
        "justfile.sublime-syntax must be installed"
    );
    std::fs::remove_dir_all(&tmp).ok();
}

#[test]
fn install_bundled_plugins_migrates_old_paths() {
    let _guard = ENV_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let tmp = fresh_plugin_dir();
    std::fs::create_dir_all(&tmp).unwrap();
    let config_home_var = if cfg!(windows) {
        "APPDATA"
    } else {
        "XDG_CONFIG_HOME"
    };
    let old = std::env::var_os(config_home_var);
    unsafe { std::env::set_var(config_home_var, &tmp) };

    let plugins_dir = tmp.join("mantis").join("plugins");
    std::fs::create_dir_all(&plugins_dir).unwrap();

    // Create old binaries
    let old_markdown_name = if cfg!(windows) {
        "mantis-plugin-markdown.exe"
    } else {
        "mantis-plugin-markdown"
    };
    let old_iconize_name = if cfg!(windows) {
        "mantis-plugin-iconize.exe"
    } else {
        "mantis-plugin-iconize"
    };
    std::fs::write(plugins_dir.join(old_markdown_name), b"old md binary").unwrap();
    std::fs::write(plugins_dir.join(old_iconize_name), b"old iconize binary").unwrap();

    // Run install, which should clean up retired/old binaries and write new ones
    install_bundled_plugins();

    unsafe {
        match old {
            Some(v) => std::env::set_var(config_home_var, v),
            None => std::env::remove_var(config_home_var),
        }
    }

    // Verify old binaries were deleted
    assert!(
        !plugins_dir.join(old_markdown_name).exists(),
        "old markdown binary must be deleted"
    );
    assert!(
        !plugins_dir.join(old_iconize_name).exists(),
        "old iconize binary must be deleted"
    );

    // Verify new binaries exist
    let new_markdown_name = if cfg!(windows) {
        "markdown.exe"
    } else {
        "markdown"
    };
    let new_iconize_name = if cfg!(windows) {
        "iconize.exe"
    } else {
        "iconize"
    };
    assert!(
        plugins_dir.join(new_markdown_name).exists(),
        "new markdown binary must exist"
    );
    assert!(
        plugins_dir.join(new_iconize_name).exists(),
        "new iconize binary must exist"
    );

    // Verify config entry path migration
    let toml_str = r#"
[plugins.markdown]
enabled = false
path = "mantis-plugin-markdown"

[plugins.iconize]
enabled = true
path = "mantis-plugin-iconize.exe"
"#;
    let mut config: crate::config::Config = toml::from_str(toml_str).unwrap();
    config.migrate_legacy_plugin_paths();

    let md_entry = config.plugins.get("markdown").unwrap();
    assert_eq!(md_entry.path.to_str().unwrap(), "markdown");
    assert!(!md_entry.enabled);

    let ic_entry = config.plugins.get("iconize").unwrap();
    assert!(
        ic_entry.path.to_str().unwrap() == "iconize"
            || ic_entry.path.to_str().unwrap() == "iconize.exe"
    );
    assert!(ic_entry.enabled);

    // Verify config entry key migration
    let toml_str_key = r#"
[plugins.mantis-plugin-markdown]
enabled = false
path = "mantis-plugin-markdown"
"#;
    let mut config_key: crate::config::Config = toml::from_str(toml_str_key).unwrap();
    config_key.migrate_legacy_plugin_paths();
    let md_entry_key = config_key.plugins.get("markdown").unwrap();
    assert_eq!(md_entry_key.path.to_str().unwrap(), "markdown");
    assert!(!md_entry_key.enabled);

    std::fs::remove_dir_all(&tmp).ok();
}

#[test]
fn install_bundled_plugins_atomic_replace_preserves_executable_permissions() {
    let _guard = ENV_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let tmp = fresh_plugin_dir();
    std::fs::create_dir_all(&tmp).unwrap();
    let var = config_home_var();
    let old = std::env::var_os(var);
    unsafe { std::env::set_var(var, &tmp) };

    let plugins_dir = tmp.join("mantis").join("plugins");
    std::fs::create_dir_all(&plugins_dir).unwrap();

    let iconize_name = if cfg!(windows) {
        "iconize.exe"
    } else {
        "iconize"
    };
    let iconize_path = plugins_dir.join(iconize_name);
    std::fs::write(&iconize_path, b"dummy old iconize").unwrap();

    install_bundled_plugins();

    assert!(iconize_path.exists());
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let perms = std::fs::metadata(&iconize_path).unwrap().permissions();
        assert_eq!(perms.mode() & 0o111, 0o111, "binary must be executable");
    }

    unsafe {
        match old {
            Some(v) => std::env::set_var(var, v),
            None => std::env::remove_var(var),
        }
    }
    std::fs::remove_dir_all(&tmp).ok();
}
