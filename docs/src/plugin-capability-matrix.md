# Plugin Capability Matrix

Audit of the plugin protocol (v2) as of 0.12.x: what the protocol declares,
what the host actually implements, and what the bundled plugins use. This is
the "take stock" pass after the 0.8 plugin work (issue #296). For the protocol
itself — message formats, examples, contribution/teardown rules — see
[Plugin Development](plugin-development.md).

## Capabilities

Capabilities are declared by a plugin in its `register_language_provider`
action and stored as `Capability` variants (`src/plugin/types.rs`). The host
routes them via `PluginManager::provider_for` (`src/plugin/manager.rs`).

| Capability | Declared in protocol | Handled by host | Used by a bundled plugin |
|---|---|---|---|
| `fold` | yes | yes — gates `set_fold_regions` in `handle_plugin_set_fold_regions` (`src/app/refresh.rs`) | **yes** — used by the bundled `rust`, `go`, `python`, `json`, `sh`, `yaml`, and `terraform` language provider plugins |
| `status_facts` | yes (0.18.x) | yes — gates `set_status_facts` in `handle_plugin_set_status_facts` (`src/app/refresh.rs`) | **yes** — used by the bundled `k8s` plugin, coexisting with `yaml`'s `fold` registration on the same `.yaml`/`.yml` extensions |
| `highlight` | yes | **no** — accepted at registration, never checked anywhere | no |
| `hover` | yes (reserved) | no — unimplementable in v2 (no request/response correlation) | no |
| `diagnostics` | yes (reserved) | no — same as `hover` | no |
| `definition` | yes (reserved) | no — same as `hover` | no |

Note on `highlight`: real syntax highlighting flows through **syntax plugins**
(`kind = "syntax"`, a `.sublime-syntax` file fed to syntect — see the bundled
`toml`/`typescript`/`dockerfile`/`nginx`/`justfile` plugins), not through
language-provider capabilities. The
`highlight` capability is documented as reserved for future provider-driven
highlighting; today registering it has no effect.

## Actions

Every action the host accepts, dispatched in `App::handle_plugin_action`
(`src/app/refresh.rs`). Unknown actions are silently ignored.

| Action | Handled by host | Contribution tracked | Torn down | Used by a bundled plugin |
|---|---|---|---|---|
| `show_message` | yes | no (transient status text) | n/a — `plugin_message` may briefly outlive the plugin; harmless | no |
| `open_file` | yes | no (one-shot navigation) | n/a | no |
| `set_icon_map` | yes | `has_icon_map` | yes — icon map/fields cleared | yes — iconize |
| `set_content` | yes | `content_paths` | yes — content removed, current file re-rendered | yes — markdown |
| `register_language_provider` | yes | provider registration in `PluginManager` | yes — `remove_provider_registrations` | yes — rust, python, json, yaml, k8s |
| `set_fold_regions` | yes | `fold_region_paths` | yes — regions removed, fold state reset | yes — rust, python, json, yaml |
| `set_status_facts` | yes | `status_fact_paths` | yes — facts removed for contributed paths | yes — k8s |

Teardown status: **every stateful `set_*` action stamps `PluginContributions`
and is cleared by `App::teardown_plugin_contributions`** (`src/app/mod.rs`).
No teardown gaps were found in this audit.

Protocol v1 git actions (`set_file_statuses`, `set_blame_data`,
`set_status_bar_git_info`) were removed in 0.11.22 along with the retired
shell-script git plugins; git features are built in. They are listed in the
version history in [Plugin Development](plugin-development.md) only.

## Bundled plugins

| Plugin | Kind | Actions sent | Capabilities registered |
|---|---|---|---|
| `iconize` | process | `set_icon_map` | none |
| `markdown` | process | `set_content` | none |
| `python` | process | `register_language_provider`, `set_fold_regions` | `fold` |
| `rust` | process | `register_language_provider`, `set_fold_regions` | `fold` |
| `go` | process | `register_language_provider`, `set_fold_regions` | `fold` |
| `json` | process | `register_language_provider`, `set_fold_regions` | `fold` |
| `sh` | process | `register_language_provider`, `set_fold_regions` | `fold` |
| `yaml` | process | `register_language_provider`, `set_fold_regions` | `fold` |
| `k8s` | process | `register_language_provider`, `set_status_facts` | `status_facts` |
| `terraform` | process | `register_language_provider`, `set_fold_regions` | `fold` |

## Gaps and follow-ups

1. **Reserved capabilities (`hover`, `diagnostics`, `definition`) are
   unimplementable in protocol v2** — they need id-correlated
   request/response. Tracked in the protocol v3 proposal (issue #481), which
   names this audit as its precursor.
2. **The language-provider fold pipeline has bundled consumers** —
   `register_language_provider` + `Capability::Fold` + `set_fold_regions` are
   used by the bundled `rust` (issue #599), `go` (issue #600), `python`
   (issue #601), `json` (issue #604), `sh` (issue #605), `yaml`
   (issue #603), and `terraform` (issue #782) language
   provider plugins. The `rust` and `go` plugins register the `fold`
   capability for `.rs` and `.go` files via the shared `brace_fold` detector
   (#598); the `python` plugin uses the shared `indent_fold` detector; the
   `json` plugin uses `brace_fold_with_brackets`, a `brace_fold` variant that
   also folds `[…]` arrays; the `sh` plugin uses `shell_brace_fold`, a
   shell-specific variant that handles `#` line comments, single/double
   quoted strings, and heredocs; the `yaml` plugin uses the shared `yaml_fold`
   detector — the same algorithm the built-in
   `crate::yaml_fold::detect_fold_regions` re-exports, so plugin-enabled and
   built-in YAML folding agree; the `terraform` plugin uses `hcl_brace_fold`,
   an HCL-specific variant that handles `#`/`//`/`/* */` comments,
   double-quoted strings, and heredocs for `.tf`, `.tfvars`, and `.hcl`
   files. Built-in YAML folding (`compute_file_load` in
   `src/app/loader.rs`) is unchanged in this phase — it still dispatches to
   `yaml_fold::detect_fold_regions` directly, and the plugin's regions only
   take over when the plugin is enabled (existing override precedence in
   `handle_plugin_set_fold_regions`). Retiring the built-in dispatch is a
   separate follow-up (issue #603 phase 2), deferred until bundled plugins
   have a default-enabled mechanism.
   **Known limitation:** provider routing is extension-based; extensionless
   scripts with a `#!/bin/bash` shebang won't route to the plugin. Shebang
   routing is a host/protocol gap (see #605).
3. **`Capability::Highlight` is declared but routes to nothing.** Either
   implement provider-driven highlighting in v3 or re-document it as reserved
   alongside `hover`/`diagnostics`/`definition`. Not yet tracked in a
   dedicated issue; candidate checklist item for #481.
4. **`status_facts`/`set_status_facts` (0.18.x) resolves the protocol gap
   that blocked issue #606** (k8s manifest awareness): the epic's "per-language
   statusbar facts" item (originally #482, merged into #602) had no protocol
   surface, and #606 was filed as a proposal rather than a build order for
   exactly that reason. The new capability is deliberately generic free text,
   not a structured breadcrumb — the bundled `k8s` plugin uses it for both
   candidate features from #606 (resource identity and multi-doc per-kind
   counts) combined into one string, rather than adding a second `breadcrumb`
   capability. **Not yet solved:** per-cursor "which resource is the viewport
   in right now" needs the host to send line/cursor position on
   `on_selection_change`, which the protocol still doesn't carry — the `k8s`
   plugin reports the first resource in the file instead of the one under the
   cursor. That's a separate, still-open protocol gap.
