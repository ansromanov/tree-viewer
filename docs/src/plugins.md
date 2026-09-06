# Plugins

`mantis` supports two kinds of plugins: **process plugins** (subprocess-based) and
**syntax plugins** (syntax definitions loaded into the highlighter).

Process plugins are standalone executables that hook into app events and issue
actions back to the viewer. They run in separate processes; `mantis` talks to them
over stdin/stdout using newline-delimited JSON, so a plugin can be any
executable — a compiled binary, a Python script, or anything that can read
stdin and write stdout.

Syntax plugins provide `.sublime-syntax` files that are loaded into the built-in
syntect highlighter at startup. They add syntax highlighting for new file types
without modifying the core binary.

## Installing a plugin

Plugins live in the default plugin directory:

- **Linux / macOS:** `~/.config/mantis/plugins/`
  (or `$XDG_CONFIG_HOME/mantis/plugins/` if that variable is set)
- **Windows:** `%APPDATA%\mantis\plugins\`

### Process plugins

Drop the executable there and make it executable (`chmod +x my-plugin.sh` on
Unix).

### Syntax plugins

`.sublime-syntax` files placed in `{plugin_dir}/syntaxes/` are auto-discovered
at startup. They are installed automatically by `mantis`'s first-run setup.

## Registering a plugin

### Process plugins

Add a `[plugins]` section to your `mantis.toml`:

```toml
[plugins]
my-plugin = { path = "my-plugin.sh", enabled = true }
```

- **`path`** — path to the executable. Relative paths are resolved against the
  default plugin directory above. Absolute paths are used as-is.
- **`enabled`** — set to `false` to keep the entry without loading the plugin.

You can register as many plugins as you like:

```toml
[plugins]
git-stats   = { path = "git-stats.sh" }
file-logger = { path = "/usr/local/bin/mantis-file-logger" }
dev-tools   = { path = "dev-tools.py", enabled = false }
```

### Syntax plugins

Syntax plugins can also be registered explicitly in `mantis.toml` with
`kind = "syntax"`:

```toml
[plugins]
toml = { kind = "syntax", syntax_file = "syntaxes/toml.sublime-syntax",
         extensions = ["toml"] }
typescript = { kind = "syntax", syntax_file = "syntaxes/typescript.sublime-syntax",
               extensions = ["ts", "tsx", "mts", "cts", "jsx"] }
dockerfile = { kind = "syntax", syntax_file = "syntaxes/dockerfile.sublime-syntax",
               extensions = ["dockerfile"] }
nginx = { kind = "syntax", syntax_file = "syntaxes/nginx.sublime-syntax" }
justfile = { kind = "syntax", syntax_file = "syntaxes/justfile.sublime-syntax" }
```

- **`kind`** — `"syntax"` for syntax-definition plugins (default: `"process"`).
- **`syntax_file`** — path to the `.sublime-syntax` file. Relative paths are
  resolved against the plugin directory.
- **`extensions`** — file extensions this syntax should match (optional; the
  syntax definition itself also declares its own extensions).

Syntax plugins are not spawned as subprocesses. Their syntax definitions are
loaded into the highlighter alongside the built-in language set.

## What plugins can do

Plugins receive lifecycle and hook events from `mantis` and can respond with
**actions**:

| Action | Effect |
|---|---|---|
| `show_message` | Displays a message in the status bar |
| `open_file` | Opens a file in the content panel |
| `set_content` | Replaces content panel with ANSI-escaped lines |
| `set_icon_map` | Sets file-type icon glyphs (requires Nerd Font) |
| `register_language_provider` | Declares file extensions and capabilities |
| `set_fold_regions` | Provides fold regions for a file |
| `set_status_facts` | Provides a short status-bar summary for a file |
| `register_commands` | Adds commands to the Ctrl-P command palette |

Each action has specific parameters; see [Plugin Development](plugin-development.md)
for the full protocol reference.

## Protocol version

`mantis` and plugins communicate over an IPC protocol identified by a version
string. Each plugin declares its protocol version in `plugin.toml` via the
`tv_protocol` field. At startup `mantis` validates that every discovered plugin
matches the host protocol version — plugins with a mismatched version are
silently skipped. The `init` event sent to each plugin includes the host
protocol version so the plugin can verify compatibility dynamically.

Current protocol version: **`"2"`** (bumped from `"1"` for the 0.8 release).

> **Upgrading from 0.7:** Plugins written for protocol `"1"` must update
> `tv_protocol = "2"` in their `plugin.toml` and handle the new
> `protocol_version` field on the `init` event to remain compatible with 0.8+.

## Lifecycle

1. `mantis` starts, reads `[plugins]` from config, and spawns each enabled plugin.
2. Each plugin receives an `init` event (with the active theme name and host
   protocol version).
3. As you use `mantis`, plugins receive hook events (`on_file_open`,
4. When you quit, each plugin receives `on_quit` then `shutdown`, and `mantis`
   waits for each subprocess to exit.

Plugin stderr is discarded (`/dev/null`). Write debug output to a log file instead.

## Example: status-bar clock

A minimal shell plugin that shows the current time in the status bar on every
file open:

```bash
#!/usr/bin/env bash
# ~/.config/mantis/plugins/clock.sh

while IFS= read -r line; do
    event=$(echo "$line" | python3 -c "import sys,json; print(json.load(sys.stdin).get('event',''))")
    case "$event" in
        on_file_open|init)
            ts=$(date +"%H:%M:%S")
            printf '{"event":"action","action":"show_message","params":{"message":"%s"}}\n' "$ts"
            ;;
        shutdown) exit 0 ;;
    esac
done
```

Register it:

```toml
[plugins]
clock = { path = "clock.sh" }
```

## Bundled plugins

`mantis` ships with several bundled Rust plugins. Each is a workspace member
compiled alongside `mantis` and installed on first run.

| Plugin | Binary | What it does |
|---|---|---|
| iconize | `iconize` | On `init`, sends a `set_icon_map` action with Nerd Font glyphs for ~80 file extensions. Requires `icons = true` in `mantis.toml` and a Nerd Font terminal. |
| markdown | `markdown` | Renders `.md` files using pulldown-cmark, sending the output as ANSI-escaped lines via `set_content`. Responds to theme changes and `M` keypress for raw/rendered toggle. |
| python | `python` | Registers as a language provider for `.py` files with the `fold` capability. On file open, computes and registers collapsible indentation-based fold regions. |
| rust | `rust` | Registers as a language provider for `.rs` files with the `fold` capability. On file open, computes and registers collapsible curly-brace fold regions. |
| go | `go` | Registers as a language provider for `.go` files with the `fold` capability. On file open, computes and registers collapsible curly-brace fold regions. |
| json | `json` | Registers as a language provider for `.json` files with the `fold` capability. On file open, computes fold regions for multi-line objects and arrays in the displayed JSON. |
| sh | `sh` | Registers as a language provider for `.sh`, `.bash`, and `.zsh` files with the `fold` capability. On file open, computes fold regions for function bodies and compound blocks, aware of `#` comments, quoted strings, and heredocs. |
| yaml | `yaml` | Registers as a language provider for `.yaml`/`.yml` files with the `fold` capability. On file open, computes and registers collapsible indentation-based fold regions. When enabled, its regions take precedence over the built-in YAML folding. |
| k8s | `k8s` | Registers as a language provider for `.yaml`/`.yml` files with the `status_facts` capability — coexists with the `yaml` plugin's `fold` registration on the same extensions since they declare different capabilities. On file open, heuristically detects Kubernetes manifests (`apiVersion:` + `kind:`) and reports the first resource's identity (`Kind/name (namespace)`) plus per-kind counts across `---`-separated documents (e.g. `Deployment/nginx (default) · 3 Deployments · 2 Services · 1 ConfigMap`) in the status bar. Files without both `apiVersion` and `kind` show no facts. |
| terraform | `terraform` | Registers as a language provider for `.tf`, `.tfvars`, and `.hcl` files with the `fold` capability. On file open, computes fold regions for HCL blocks via the shared `hcl_brace_fold` detector, which is aware of `#`, `//`, and `/* */` comments, double-quoted strings, and heredocs. Highlighting for these files comes from the bundled `terraform.sublime-syntax`, auto-discovered from the `syntaxes/` directory. |

### Bundled syntax plugins

`mantis` also bundles syntax-only plugins that provide highlighting for
additional file types without spawning a subprocess.

| Plugin | Extensions | What it highlights |
|---|---|---|
| toml | `.toml` | TOML configuration files (`Cargo.toml`, `mantis.toml`, `pyproject.toml`, etc.) |
| typescript | `.ts`, `.tsx`, `.mts`, `.cts`, `.jsx` | TypeScript and TSX (JSX) source files |
| dockerfile | `Dockerfile`, `Containerfile` | Dockerfile instructions (matched by filename, no extension) |
| nginx | `nginx.conf` | Nginx server configuration (matched by filename, no extension claim) |
| justfile | `justfile` | Just (justfile) task recipes (matched by filename, no extension) |

Terraform/HCL highlighting is handled by the same `terraform.sublime-syntax`
file, but as an auto-discovered syntax definition rather than a `[plugins]`
entry — see the `terraform` process plugin row above.

All bundled plugins are compiled as workspace members and installed to the
plugin directory the first time `mantis` creates its global config. They are all
enabled by default. If you want to disable a bundled plugin, explicitly set
`enabled = false` in your `mantis.toml`:

```toml
[plugins.rust]
enabled = false
```

> **Note:** Git features are built into `mantis` natively — no plugin required.
> Git status colors, blame, file history (`H`, while the tree is focused), and
> working-tree diffs all work without any plugin enabled.

### Auto-removal of retired plugins

When upgrading from a pre-0.8 install (which used shell-script plugins), superseded
files (`git-diff.sh`, `git-log.sh`, `iconize.sh`) and their `[plugins]` config entries
are silently removed on first launch. User-authored plugins are never touched.
