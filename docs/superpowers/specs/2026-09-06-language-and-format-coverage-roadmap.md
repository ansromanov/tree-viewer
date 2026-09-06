# Language & Format Coverage Roadmap (2026-09)

> **Status:** Living roadmap & architectural spec
> **Parent Epic:** [#602](https://github.com/ansromanov/mantis/issues/602)
> **Target Scope:** Core programming languages, Web frontends, Infrastructure as Code, Shells, and Config/Template formats (excluding domain-specific audio/music binary formats).

---

## 1. Vision & Background

Mantis is designed as a high-performance terminal reading cockpit for developers and AI agents.
An audit of real-world multi-repo environments identified that backend, DevOps, and full-stack development span a focused set of recurring formats:

1. **Systems & Backend:** Rust (`.rs`), Go (`.go`), Python (`.py`).
2. **Web & Frontend:** TypeScript / TSX (`.ts`, `.tsx`), JavaScript / JSX (`.js`, `.jsx`, `.mjs`), HTML (`.html`), CSS (`.css`).
3. **Infrastructure & DevOps:** Terraform / OpenTofu (`.tf`, `.tfvars`), HashiCorp HCL (`.hcl`), Systemd / INI (`.ini`, `.service`, `.timer`, `.conf`), Dockerfile (`Dockerfile`), Nginx (`nginx.conf`), Justfile (`justfile`).
4. **Shells & Scripting:** POSIX / Bash / Zsh (`.sh`, `.bash`, `.zsh`), PowerShell (`.ps1`, `.psm1`, `.powershell`).
5. **Data, Serialization & Queries:** JSON / JSONC / JSONL (`.json`, `.jsonc`, `.jsonl`), YAML (`.yaml`, `.yml`), TOML (`.toml`), SQL (`.sql`).
6. **Templates:** Helm / Go-templates / Jinja2 (`.tpl`, `.tmpl`, `.template`, `.mako`, `.ejs`).

### Architectural Division of Labor

- **Host Core (`mantis` library crate):** Provides pure, zero-dependency algorithmic detectors (`brace_fold`, `brace_fold_with_brackets`, `shell_brace_fold`, `indent_fold`, `yaml_fold`, and future `section_fold`, `template_fold`, `sql_fold`, symbol extractors).
- **Plugin Layer (Protocol v3):** Carries per-language integrations (`register_language_provider`, capabilities `fold`, `status_facts`, future `symbol_outline`).
- **Syntax Packs:** Delivers `.sublime-syntax` definitions for languages not bundled in Syntect's default syntax set.

---

## 2. Current State & Target Matrix

| Language / Format | Extensions / Files | Syntax Highlighting | Code Folding | Provider / Plugin | Status / Issue |
|---|---|---|---|---|---|
| **Rust** | `.rs` | ✅ Syntect built-in | ✅ `brace_fold` | `plugins/rust` | Shipped (#599) |
| **Go** | `.go` | ✅ Syntect built-in | ✅ `brace_fold` | `plugins/go` | Shipped (#600) |
| **Python** | `.py`, `.pyi` | ✅ Syntect built-in | ✅ `indent_fold` | `plugins/python` | Shipped (#601) |
| **Shell (POSIX/Bash/Zsh)** | `.sh`, `.bash`, `.zsh` | ✅ Syntect built-in | ✅ `shell_brace_fold` | `plugins/sh` | Shipped (#605) |
| **JSON** | `.json` | ✅ Syntect built-in | ✅ `brace_fold_with_brackets` | `plugins/json` | Shipped (#604) |
| **YAML** | `.yaml`, `.yml` | ✅ Syntect built-in | ✅ `yaml_fold` | `plugins/yaml` + `plugins/k8s` | Shipped (#603, #606) |
| **Markdown** | `.md` | ✅ Syntect built-in | ❌ (Plain text) | `plugins/markdown` | Shipped (Rich ANSI viewer) |
| **TypeScript / TSX / JSX** | `.ts`, `.tsx`, `.jsx`, `.mts` | ✅ Bundled syntax pack | 💤 Planned (`brace_fold`) | `plugins/typescript` | Tracked |
| **JavaScript** | `.js`, `.mjs`, `.cjs` | ✅ Syntect built-in | 💤 Planned (`brace_fold`) | `plugins/typescript` / `plugins/javascript` | Tracked |
| **Terraform & HCL** | `.tf`, `.tfvars`, `.hcl` | ✅ Bundled `.tf` + `.hcl` | ✅ `hcl_brace_fold` | `plugins/terraform` | Shipped (#782) |
| **INI & Systemd** | `.ini`, `.service`, `.timer`, `.conf`, `.properties` | ✅ Syntect built-in | 💤 Planned (`section_fold`) | `plugins/ini` | Tracked |
| **SQL** | `.sql` | ✅ Syntect built-in | 💤 Planned (`sql_fold`) | `plugins/sql` | Tracked |
| **PowerShell** | `.ps1`, `.psm1`, `.powershell` | 💤 Needs `.sublime-syntax` | 💤 Planned (`brace_fold`) | `plugins/powershell` | Tracked |
| **Templates (Helm/Jinja)** | `.tpl`, `.tmpl`, `.template` | 💤 Needs `.sublime-syntax` | 💤 Planned (`template_fold`) | `plugins/template` | Tracked |
| **CSS** | `.css`, `.scss`, `.less` | ✅ Syntect built-in | 💤 Planned (`brace_fold`) | `plugins/css` | Tracked |
| **TOML** | `.toml` | ✅ Bundled syntax pack | 💤 Planned (`section_fold`) | `plugins/toml` | Tracked |
| **HTML** | `.html`, `.htm` | ✅ Syntect built-in | 💤 Planned (tag-matching fold) | `plugins/html` | Proposed |
| **Dockerfile** | `Dockerfile`, `*.dockerfile` | ✅ Bundled syntax pack | 💤 Planned (`FROM` stage fold) | `plugins/dockerfile` | Proposed |
| **Nginx** | `nginx.conf` | ✅ Bundled syntax pack | 💤 Planned (`brace_fold`) | `plugins/nginx` | Proposed |
| **Justfile** | `justfile`, `Justfile` | ✅ Bundled syntax pack | 💤 Planned (recipe fold) | `plugins/justfile` | Proposed |

---

## 3. Capability Milestones

### Milestone 1: Syntax & Folding for Priority Infrastructure & Web Formats

1. **TypeScript & JavaScript Folding:**
   - Extend `plugins/typescript` to register `language_provider` with `fold` capability for `ts`, `tsx`, `js`, `jsx`, `mjs`, `cjs`.
   - Reuses `mantis::fold_detectors::brace_fold`.

2. **Terraform & HashiCorp HCL (`tf`, `hcl`):**
   - Add `.hcl` mapping to `plugins/terraform/syntaxes/terraform.sublime-syntax`.
   - Upgrade `plugins/terraform` to a language provider registering `fold` for `tf`, `tfvars`, `hcl` using `brace_fold` with `#` and `//` comment skipping.

3. **INI, Systemd & Conf Section Folding (`ini`):**
   - Implement `pub fn section_fold(text: &str) -> Vec<FoldRegion>` in `src/fold_detectors.rs` (folds `[section]` blocks up to next section header or EOF).
   - Create bundled `plugins/ini` covering `.ini`, `.service`, `.timer`, `.conf`, `.properties`, `.cfg`.

4. **SQL Statement & Block Folding (`sql`):**
   - Implement `pub fn sql_fold(text: &str) -> Vec<FoldRegion>` in `src/fold_detectors.rs` (handles `BEGIN...END`, `CREATE FUNCTION/PROCEDURE/TABLE`, `CASE...END`, multiline statements).
   - Create bundled `plugins/sql` for `.sql`.

5. **PowerShell Syntax & Folding (`ps1`):**
   - Add bundled `powershell.sublime-syntax` for `.ps1`, `.psm1`, `.psd1`, `.powershell`.
   - Register `fold` capability via `brace_fold`.

6. **Template Engine Syntax & Folding (`tpl`):**
   - Add bundled `.sublime-syntax` for Go-template / Jinja2 / Helm templates (`.tpl`, `.tmpl`, `.template`).
   - Implement template block fold detector (`{{ define ... }}...{{ end }}`, `{{ if ... }}...{{ end }}`).

7. **CSS & TOML Folding:**
   - Wire `brace_fold` for `.css`, `.scss`, `.less`.
   - Wire `section_fold` for `.toml` tables (`[table]` and `[[array.of.tables]]`).

---

### Milestone 2: Symbol Extraction & Go-To-Symbol Picker

- Add lightweight regex-based symbol extraction in `src/symbol_extractors.rs` (pure algorithms):
  - Rust: `fn`, `struct`, `enum`, `trait`, `impl`, `type`, `const`.
  - Go: `func`, `type`, `interface`, `struct`.
  - Python: `def`, `class`.
  - TypeScript/JS: `function`, `class`, `interface`, `type`, `const/let` exports.
- Protocol surface: Add `symbol_outline` capability to request/response surface.
- UI: Command palette / fuzzy picker overlay for quick symbol jumps (`@symbol` or `Ctrl-R`).

---

### Milestone 3: Breadcrumb Scope Context & Sticky Scroll

- Use the symbol table to resolve the enclosing function/class/scope for the current viewport cursor line.
- Render enclosing scope in the status bar / pane header breadcrumb.
- Supply scope boundaries for sticky scroll ([#199](https://github.com/ansromanov/mantis/issues/199)).

---

### Milestone 4: Per-Language Status Facts

- Protocol surface shipped in 0.18.x (`status_facts` capability + `set_status_facts` action).
- Expand beyond `k8s` to core language providers:
  - Rust: `X fns · Y structs · Z pubs`
  - Python: `X classes · Y defs`
  - Go: `X funcs · Y types`
  - SQL: `X tables · Y procedures · Z queries`
