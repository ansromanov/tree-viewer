# AGENTS.md — mantis

> Canonical instructions for this repo, read by both Claude Code and opencode.
> Agent assets live in **`.agent/`**, which is symlinked as both `.claude/` and
> `.opencode/` so the two tools share one config + skills directory. `CLAUDE.md`
> defers here and adds only Claude-Code-specific notes.

A fast terminal-based file tree viewer built with ratatui. Navigate filesystems,
preview files with syntax highlighting (`syntect`), fuzzy-search files/content
(`fuzzy-matcher`), browse git history
(`git` CLI), and switch themes — all with mouse and keyboard.

---

# 1. Project Structure

Single crate, no workspace. Library code in `src/lib.rs`; the `mantis` binary in
`src/main.rs`. Tests are co-located in `_test.rs` files (see Rust Guidelines →
Testing).

```
src/
├── main.rs                         # Entry: terminal setup, sync event loop, dispatch
├── lib.rs                          # Module declarations (crate root)
├── app/
│   ├── mod.rs                      # App struct + shared free functions
│   ├── content_pos.rs              # Content-pane geometry/scroll math (viewport, gutter)
│   ├── content_query.rs            # Read-only line-count/line-text queries (VirtualFile vs raw vs MD vs JSON)
│   ├── diff_nav.rs                 # Jump between @@ hunk headers in diffs
│   ├── file_ops.rs                 # Open/close/reveal file operations
│   ├── key_handlers/
│   │   ├── mod.rs                  # Top-level key dispatch (overlay chain → mode → panel)
│   │   ├── editor.rs               # Key handling when in text-edit/insert mode
│   │   ├── normal.rs               # Key handling in normal (tree/content) mode
│   │   ├── overlay.rs              # Key handling for all overlay popups
│   │   └── visual.rs               # Key handling in visual-selection mode
│   ├── loader.rs                   # Background file-loader thread (LoadRequest/LoadResponse)
│   ├── mouse_handlers.rs           # Mouse event dispatch and click hit-testing
│   ├── navigation.rs               # Tree cursor movement helpers
│   ├── refresh.rs                  # Per-frame tick: drain loads, watcher, debounced search
│   ├── fold.rs                     # Fold state management (generic regions + language-provider override)
│   └── *_test.rs                   # Co-located tests
├── ui/
│   ├── mod.rs                      # ratatui rendering orchestration (draw entry point)
│   ├── content/
│   │   ├── mod.rs                  # Content panel draw entry point
│   │   ├── diff.rs                 # Side-by-side and unified diff rendering
│   │   ├── draw.rs                 # Core content rendering (lines, gutter, highlights)
│   │   ├── scrollbar.rs            # Transient fade scrollbar overlay
│   │   ├── search.rs               # In-file search match highlight overlay
│   │   └── selection.rs            # Visual-selection highlight overlay
│   ├── popups/
│   │   ├── mod.rs                  # Re-exports all popup draw functions
│   │   ├── about.rs                # About + release notes overlay
│   │   ├── blame.rs                # Git blame panel for visual-selection range
│   │   ├── command.rs              # Ctrl-P command palette popup
│   │   ├── help.rs                 # ? keybinding help overlay
│   │   ├── history.rs              # Git file-log history overlay
│   │   ├── in_file.rs              # / in-file search bar (bottom of content pane)
│   │   ├── plugin.rs               # Plugin manager overlay
│   │   ├── recent.rs               # Recent-files overlay
│   │   ├── search.rs               # Fuzzy file/content search overlay
│   │   ├── theme.rs                # Theme picker overlay
│   │   └── util.rs                 # Shared popup layout helpers
│   ├── statusbar.rs                # Status bar rendering
│   ├── tree.rs                     # Tree panel rendering
│   └── *_test.rs                   # Co-located tests
├── config/
│   ├── mod.rs                      # mantis.toml deserialization, keybinding parsing
│   └── validate.rs                 # Config validation
├── ansi.rs                         # ANSI SGR parser → ratatui Style/Span (for plugin content)
├── command_palette.rs              # CommandPalette + CommandEntry structs and COMMANDS table
├── diff.rs                         # Git diff parse (DiffRow, Cell, parse_side_by_side)
├── file.rs                         # Binary detection, encoding/line-ending probe
├── git.rs                          # Shell-out to git: log, diff, blame, status, repo_info
├── highlight.rs                    # syntect Highlighter → ratatui Style spans
├── plugin/
│   └── mod.rs                      # Plugin, PluginManager, PluginKind, ExtraSyntax; subprocess IPC
├── release_info.rs                 # Compile-time release metadata (ReleaseInfo, RELEASE static)
├── search/
│   ├── mod.rs                      # SearchState + re-exports (SearchMode, ContentMatch)
│   ├── history.rs                  # HistoryState (git log picker)
│   └── pickers.rs                  # TreeFilter, GotoLineState, InFileSearch, ThemePicker,
│                                   #   RecentFilesState, PluginPicker
├── selection.rs                    # TextSelection + VisualLine for copy/visual mode
├── theme.rs                        # Theme struct, color roles, presets, parse_color
├── tree.rs                         # TreeNode, build_visible (flat Vec from ignore::WalkBuilder)
├── virtual_file.rs                 # Memory-mapped lazily-indexed file (VirtualFile)
├── fold.rs                         # Generic FoldRegion data model + display-map builder
└── yaml_fold.rs                    # YAML-specific fold-region detection (indentation-based)
```

Files grow into the module-directory pattern (`src/app/`, `src/ui/`, `src/config/`)
once they get large: a thin `mod.rs` re-exports focused submodules.

## Key patterns & conventions

1. **Flat tree vector.** The file tree is a `Vec<TreeNode>` with explicit `depth`;
   expansion is tracked in a `HashSet<PathBuf>`. Simpler than nested trees for
   rendering and mouse hit-testing.
2. **Overlay state machine.** Event handlers chain `help` > `about` > `plugin_picker` >
   `recent_files` > `command_palette` > `theme_picker` > `history` > `in_file_search` >
   `search` > normal dispatch (tree/content by `Focus`). The same chain appears in
   `handle_mouse()` and `draw()`.
3. **Recorded geometry for mouse.** Each `draw_*` stores its rendered `Rect` and
   scroll offset back on `App`; mouse handlers hit-test with `rect_contains()`.
   **Always account for scroll offsets in click calculations.**
4. **Fuzzy-filterable picker.** `SearchState`, `HistoryState`, `ThemePicker`,
   `RecentFilesState`, `PluginPicker`, and `CommandPalette` share a shape: query
   string, full list, filtered+scored list, selected index, `push(c)`/`pop()` →
   `refresh()`. Uses `SkimMatcherV2`, descending score sort.
5. **Semantic theming.** `Theme` is a set of named color roles (not literal colors)
   plus a `syntax` syntect theme name. Presets (default, monokai, solarized,
   catppuccin, synthwave84) live in `themes/` plus user overrides; `apply_theme()`
   re-opens the current file after a switch.
6. **Keybinding abstraction.** All actions bind through a `Keymap`; `pressed()`
    checks binding lists. Fully remappable via `mantis.toml` `[keys]`.
7. **Git via shell-out.** `git.rs` runs `git log` / `git diff` / `git blame` rather
   than linking a Rust git library, with graceful fallback on failure.
8. **Sync event loop.** `crossterm::event::poll()` with a 16ms timeout — no async
   runtime, just a synchronous tick loop.
9. **Content source abstraction.** `app/content_query.rs` hides whether the active
   content is a `VirtualFile`, `Vec<String>`, JSON pretty-print, or markdown spans;
   everything calls `app.line_count()` / `app.line_text(n)`.
10. **No alt-modified keybindings.** The Alt modifier conflicts with
    terminal-level key processing and is unreliable across terminals. Every
    default binding must use only Ctrl, Shift (via char case), or unmodified
    keys. Never introduce a new `alt+` binding.
11. **Plugin IPC.** Plugins are external processes communicating over stdin/stdout
    JSON lines (`plugin/mod.rs`). `PluginManager` spawns, kills, and routes actions;
    the content pane can be taken over by plugin-provided ANSI text (`ansi.rs`).
    State teardown is structural: every `set_*` action stamps its originating plugin
    in `plugin_contributions`, and on disable/crash the host clears exactly that
    plugin's output. **Adding a new `set_*` action requires recording it in
    `PluginContributions` and handling teardown in `teardown_plugin_contributions`.**

---

## Symbol index

Quick lookup: type/function → file. Use this before grepping.

| Symbol | File | Notes |
|---|---|---|
| `App` | `src/app/mod.rs:74` | Central state struct |
| `Focus` | `src/app/mod.rs:51` | `Tree` / `Content` enum |
| `PluginContributions` | `src/plugin/types.rs` | Plugin contribution tracking for teardown |
| `App::teardown_plugin_contributions` | `src/app/mod.rs` | Clears all state produced by a plugin |
| `App::tick` | `src/app/refresh.rs` | Per-frame update |
| `App::handle_key` | `src/app/key_handlers/mod.rs` | Top-level key dispatch |
| `App::handle_mouse` | `src/app/mouse_handlers.rs` | Mouse event dispatch |
| `App::open_file` | `src/app/file_ops.rs` | Load a file into the content pane |
| `App::line_count` / `App::line_text` | `src/app/content_query.rs` | Unified content access |
| `App::content_scroll_max` / `App::line_prefix_width` | `src/app/content_pos.rs` | Scroll/gutter math |
| `App::diff_next_hunk` / `App::diff_prev_hunk` | `src/app/diff_nav.rs` | Hunk navigation |
| `Loader` / `LoadRequest` / `LoadResponse` | `src/app/loader.rs` | Background file I/O thread |
| `TreeNode` / `build_visible` | `src/tree.rs` | Flat tree vector |
| `VirtualFile` | `src/virtual_file.rs` | Lazily-indexed file (owned ≤16MB, mmap above) |
| `SearchState` | `src/search/mod.rs` | Fuzzy file+content search |
| `HistoryState` | `src/search/history.rs` | Git file-log picker |
| `InFileSearch` | `src/search/pickers.rs` | Within-file incremental search |
| `ThemePicker` | `src/search/pickers.rs` | Theme selection overlay |
| `RecentFilesState` | `src/search/pickers.rs` | Recent-files overlay |
| `PluginPicker` | `src/search/pickers.rs` | Plugin manager overlay |
| `CommandPalette` / `CommandEntry` | `src/command_palette.rs` | Ctrl-P palette |
| `TextSelection` / `VisualLine` | `src/selection.rs` | Visual/copy selection |
| `Theme` / `parse_color` | `src/theme.rs` | Color roles + presets |
| `ThemeConfig` | `src/theme.rs:290` | TOML deserialization target |
| `Highlighter` | `src/highlight.rs:30` | syntect → ratatui styles |
| `DiffRow` / `parse_side_by_side` | `src/diff.rs` | Diff parse/render types |
| `GitRepoInfo` / `GitStatus` / `Commit` / `BlameLine` | `src/git.rs` | Git shell-out types |
| `FoldRegion` / `build_display_map` | `src/fold.rs` | Generic fold data model and display-map computation |
| `detect_fold_regions` / `count_anchors_aliases` | `src/yaml_fold.rs` | YAML-specific fold-region detection |
| `Capability` / `LanguageProviderRegistration` | `src/plugin/mod.rs` | Language provider protocol types |
| `Plugin` / `PluginManager` / `PluginKind` | `src/plugin/mod.rs` | Plugin subprocess IPC |
| `ExtraSyntax` / `PluginEntry` | `src/plugin/mod.rs` | Plugin-registered syntaxes |
| `ReleaseInfo` / `RELEASE` | `src/release_info.rs` | Embedded release metadata |
| `parse_ansi_line` | `src/ansi.rs` | ANSI SGR → ratatui Span |
| `Config` / `Keymap` | `src/config/mod.rs` | mantis.toml deserialization |
| `draw` | `src/ui/mod.rs:26` | Main ratatui draw entry point |

---

# 2. Rust Guidelines

## Code style

- **Indent** 4 spaces, no tabs. **Line length** 100 chars max.
- **Naming** snake_case for functions/vars/modules, PascalCase for types/enums.
- **Imports** grouped std → external crates → local modules, separated by blank lines.
  No wildcard imports except `use super::*;` in test files.
- **Doc comments** on all public items. No tautological or self-demonstrating comments.
- **File-level module docs.** Every non-test `.rs` file under `src/` opens with a
  10-15 line `//!` block describing what the module does, the problem it solves, and
  which public items it owns, written for a developer new to the project. Treat keeping
  this block in sync as part of any PR that changes the file's behaviour, the way a
  changelog entry is updated.
- **No emoji/unicode** in source (except test assertions exercising multi-byte handling).
- **Explicit `.clone()`** on non-Copy types — no hidden clones.

## Error handling

`anyhow` only in `main` and `App::new`; use `.context()` for actionable messages.
File and git errors degrade gracefully to UI messages — **no `unwrap`/`expect` in
production paths** (tests may use them freely). Custom errors via `thiserror`.

**No silently-swallowed user-visible failures.** Discarding an error with `let _ = …`
is fine for best-effort *caches* (session, usage stats) but not for actions the user
expects to take effect — a failed config save, a failed clipboard copy, a failed
external launch must surface a status-bar message. If an operation can fail in a way
the user would want to know about, report it; don't `let _ =` it away.

## Consistency & performance

These keep the app coherent and the render loop cheap. Reviewers enforce them.

- **Design for reuse before writing code, not after.** Before adding a new piece of
  state or interaction (a new overlay's scroll offset, a new picker, a new key-handling
  block), check whether an existing part of the app already has the same shape —
  and whether the shape is likely to recur (a fourth overlay, a fifth picker). If a
  second consumer is foreseeable, build the shared primitive first rather than
  hard-coding a single-purpose field and copy-pasting the pattern the next time it's
  needed. Concretely: a scrollable-content overlay (help, about, blame, and any future
  one) should scroll through one small `ScrollState`-style helper (offset, key/wheel
  handling, clamp-and-indicator), not a bespoke `xxx_scroll: usize` field with its own
  inline `match` in `key_handlers` and `mouse_handlers` per overlay. This is a
  judgment call, not a mandate to abstract on the first instance — don't build the
  shared helper for something that will only ever have one caller.
- **One method per behaviour — don't duplicate, share.** If two code paths do the
  "same thing" (copy to clipboard, launch an editor/browser, handle a list-picker
  overlay's keys, clamp a scroll offset), extract one helper and call it from both.
  Divergent copies drift into inconsistent behaviour and bugs. Before adding a second
  near-copy of an existing routine, factor the shared part out.
- **Uniform interaction model.** Overlays/popups share their input handling and close
  rules (Esc closes, empty-Backspace closes, click-outside closes). Scroll/cursor go
  through the canonical helpers (`content_scroll_max`, a single clamp/`set_content_scroll`
  path) — input and render must agree on the same bounds. Don't re-implement
  "scroll-into-view" or "clamp selection" ad hoc.
- **Per-frame work scales with what's visible, not with total data.** `draw_*` runs
  every frame. Never allocate or compute `O(total_nodes)` / `O(total_lines)` when only
  one screenful renders — bound the work (and allocations) to the visible window. Cache
  results that don't change between frames (highlighting, filtered index sets) keyed by
  a revision/query, and recompute only on change. Gate optional work behind its toggle
  (e.g. skip indent-guide computation when guides are off).
- **A reload/re-render must preserve view state for the same content.** Resetting scroll,
  cursor, selection, or tearing down an open overlay is only correct on a genuine content
  switch (different file/revision), not on a same-file reload, watcher tick, or plugin
  re-render. Guard those resets on an "is this actually new?" check; clamp preserved
  offsets to the new content length rather than zeroing them.
- **Bounds-checked access.** Use `.get(i)` (with an early return) for any index that
  isn't provably in range from a `0..len` loop — never raw `slice[i]` on a derived index
  (hit-test rows, restored state). Keep selection clamped after any rebuild.
- **Add a benchmark for new hot paths.** Render/parse/search code that runs per frame or
  over large inputs gets a `benches/performance.rs` case so regressions are caught.

## Testing

Tests are **co-located** with the module they cover, in a sibling `_test.rs` file —
never inline `#[cfg(test)] mod tests { ... }` blocks.

- `src/foo.rs` → tests in `src/foo_test.rs`
- `src/app/mod.rs` → tests in `src/app/mod_test.rs`

Each `_test.rs` starts with `use super::*;` and contains bare `#[test]` functions
(no `mod tests` wrapper). The source file wires it up with one line:

```rust
#[cfg(test)]
#[path = "foo_test.rs"]
mod tests;
```

When adding tests to an existing module, append to its `_test.rs`. When creating a
new module, create its `_test.rs` companion at the same time. Cross-module /
black-box tests live in the integration `tests/` directory. The `split-tests` skill
(`.agent/skills/split-tests/`) automates extracting any inline block.

**Module-split rule:** when a module is split into submodules, its `_test.rs` must
be split in the same PR. Each new submodule gets its own `_test.rs` containing the
tests for items it defines. Leaving a monolithic `_test.rs` behind while splitting
production code is a merge-blocking violation.

**Mandatory test rule:** every code change — bug fix, feature, refactor — must
include tests. There are no exceptions. If a change is untestable (e.g. pure UI
paint code), explain why in the PR description. Otherwise add or update tests in
the same commit as the code change, never as a follow-up.

**Every feature PR must include tests** that cover the new functionality at
the unit level (the modified module's `_test.rs`) and, where applicable, at the
integration level. Untested features are not complete. Reviewers should block
any PR that adds user-visible behaviour without corresponding test coverage.

## Documentation

User-facing docs live in `docs/src/`. Any PR that adds, removes, or changes a
user-visible feature (config keys, keybindings, plugin protocol, new UI modes)
**must** update the relevant doc page in the same commit. Treat doc updates the
same way as the `//!` module-doc rule: part of the PR, not a follow-up.

Key pages to consider when changing code:
- Plugin system (`src/plugin/mod.rs`, `src/config/mod.rs`) → `docs/src/plugins.md`,
  `docs/src/plugin-development.md`
- Config options (`src/config/`) → `docs/src/configuration.md`
- Keybindings (`src/config/mod.rs`, `mantis.toml`) → `docs/src/configuration.md`
- New UI features → `docs/src/usage.md` or a new page added to `docs/src/SUMMARY.md`

## File size limit

- **Code files** — ideally under **700 lines**. When a source file approaches the
  limit, split it into focused submodules using the module-directory pattern
  (`src/app/`, `src/ui/`).
- **Test files** — no hard limit, but if a code file is split, its related tests
  in the `_test.rs` companion should be split into sibling `_test.rs` files too.

---

# 3. Dev Flow

> **AI agents: never run `cargo test`, `cargo nextest run`, or any other full-suite
> command during feature development. Always use `just test-pr` instead. Running the
> full suite wastes minutes and is explicitly forbidden by project convention.**

## Commands

| Command | Action |
|---|---|
| `cargo build` / `cargo build --release` | Debug / release build |
| `cargo run -- [path]` | Run with optional path |
| `just test-pr` | **Run only tests related to your changes** (skips on broad changes — never runs full suite) |
| `cargo nextest run` | Run full test suite (use manually when `just test-pr` skips due to broad change) |
| `cargo nextest run -E 'test(foo)'` | Run tests matching a filter |
| `cargo test` | Run full suite via built-in runner (fallback if nextest unavailable) |
| `cargo check` | Type-check only |
| `cargo clippy --all-targets -- -D warnings` | Lint (must pass) |
| `cargo fmt --all` / `cargo fmt --check` | Format / check formatting |
| `just` | List all recipes |
| `pre-commit install` | Install git hooks (once after clone) |
| `pre-commit run --all-files` | Run all hooks manually |

## Worktrees (mandatory for all agents)

**Mandatory, with no exceptions: every agent or harness working in this repo — Claude Code,
Codex, opencode, Kilo Code, Antigravity, or any other — must do its work in a dedicated git
worktree, never directly in the primary checkout.** This keeps parallel agent sessions (and parallel
tasks within one session) from clobbering each other's edits or switching the branch
out from under a running build/test.

```bash
git worktree add .agent/worktrees/<branch-name> -b <branch-name> origin/main
cd .agent/worktrees/<branch-name>
```

- `.agent/worktrees/` is already gitignored (and symlinked as `.claude/worktrees` /
  `.opencode/worktrees`), so worktrees never get committed.
- Do all editing, building, and testing inside the worktree directory, not the repo
  root.
- Once the PR merges (or the branch is abandoned), remove the worktree:
  `git worktree remove .agent/worktrees/<branch-name>`.
- This is in addition to, not a replacement for, `just new` for branch creation —
  run `just new` (or `git worktree add -b`) from inside the fresh worktree's parent
  checkout as shown above.
- Claude Code subagents follow this automatically via `isolation: "worktree"` (see
  `CLAUDE.md`); this section extends the same rule to top-level/interactive sessions
  of every agent.

## Branching

Always branch from `origin/main`, never from an existing feature branch:

```bash
just new your-branch-name
```

This fetches latest main, creates the branch from `origin/main`, and installs
pre-commit hooks. If you find yourself on a branch not based on main, cherry-pick the
new commits onto a clean branch rather than rebasing through merge noise.

## Opening a PR

**Branch naming — always rename before pushing.**
The session may start on an auto-generated branch (e.g. `claude/quirky-edison-7sxvm3`).
Before pushing or creating a PR, rename it to a descriptive name that matches the work:

```bash
git branch -m <old-name> feat/<short-description>   # or fix/, chore/, docs/, …
git push -u origin feat/<short-description>
git push origin --delete <old-name> || true          # best-effort; ignore 403
```

Use the `feat/` prefix for new features, `fix/` for bug fixes, `docs/` for
documentation-only changes, and `chore/` for maintenance tasks.

Prefer the one-shot recipe — it never drops a step (fmt → related tests → push →
open PR that closes the issue):

```bash
just ship 239      # fmt, just test-pr, push, then `gh pr create` with "Closes #239"
```

Or the manual path:

```bash
just pr            # fetch origin/main, rebase, push --force-with-lease
gh pr create --title "<summary>" --body "<what + why>

Closes #<n>"        # open the PR directly (not draft) with a real body
```

Never run a bare `gh pr create` / `gh pr create --fill` that leaves the body empty
or as the auto-generated stub. The rebase step is not optional: both `just ship`
and `just pr` fetch and rebase onto fresh `origin/main` before pushing, and rebase
fails loudly on conflicts so you resolve them before the PR opens.

> **macOS credential helper note:** If `git push` fails with `Device not configured`,
> the macOS keychain isn't available (e.g. in SSH sessions). Workaround:
> ```bash
> GH_TOKEN=$(gh auth token) just pr
> ```

## PR lifecycle rules (mandatory)

These are non-negotiable — CI enforces #1, and `just` recipes make the rest
a single command. They are the difference between a PR that lands clean and one that
leaves manual cleanup behind.

1. **Every PR closes its issue.** The PR body must contain `Closes #<n>` (or
   `Fixes`/`Resolves`) so GitHub auto-closes the issue on merge. CI's `link` job
   fails the PR otherwise. Only exception: apply the `no-issue` label for genuinely
   issue-less PRs. `just ship <n>` writes this for you.
2. **Always push your work.** Code that isn't pushed doesn't exist. Finish a unit of
   work → push. `just ship` does it; if you push manually, do it before reporting the
   work done.
3. **Fix review comments on the SAME branch — never a new one.** Pushing more commits
   to the PR branch updates the PR. To return to a PR's branch use `just fix <pr>`
   (wraps `gh pr checkout`). `just new` refuses to branch when the current branch
   already has an open PR, to stop this mistake.
4. **Resolve threads after addressing comments.** Fixing the code is half the job;
   the reviewer still sees an open thread until it's resolved. Run
   `just resolve-threads` (or `just ship`, which resolves automatically when updating
   an existing PR) after pushing the fixes.
5. **Always rebase onto fresh `origin/main` before pushing or opening a PR.** Never
   push a branch built on stale main. `just ship` and `just pr` run
   `git fetch origin && git rebase origin/main` for you; if you push by hand, run
   `just pr` first. Resolve rebase conflicts before the PR opens — never open a PR
   that needs a merge from main to be reviewable.
6. **Every PR has a descriptive body, opened directly (not draft).** The body must
   say *what changed and why*, not just the close directive, and never be empty or
   the bare auto-stub. `just ship <n>` derives one bullet per branch commit and
   appends `Closes #<n>`; for a written summary pass `PR_BODY="…" just ship <n>`.
   PRs open ready for review — never as drafts.

## Before committing

1. `cargo fmt --all` — formatting clean (enforced by pre-commit)
2. `cargo clippy --all-targets -- -D warnings` — no warnings (enforced by pre-commit)
3. `just test-pr` — related tests pass (**never run the full suite for a single PR**).
   It first runs `scripts/require-tests.sh`, which **fails** if any changed
   `src/**.rs` module has no sibling `_test.rs` in the diff — so a missing test file
   is caught here, in your own ship loop, not just at commit-time or in CI.
4. `cargo check` — no type errors (enforced by pre-commit)
5. Every changed module has an accompanying sibling `_test.rs` change — no untested
   diffs. Genuinely untestable (UI-paint-only) changes need `[skip-tests: <reason>]`
   in a commit message. Enforced by `just test-pr`, pre-commit, and CI.
6. No debug `println!`, `dbg!`, or commented-out code
7. No hardcoded secrets or credentials

### How `just test-pr` works

`just test-pr` diffs your branch against `origin/main`, pipes the changed file
list through `scripts/related-tests.sh`, and runs only the matching tests with
`cargo nextest`. If the changes touch broad files (`Cargo.toml`, `src/lib.rs`,
etc.) it prints a notice and skips — run `cargo nextest run` manually if you
need full coverage for those changes.

Do **not** run `cargo nextest run` (full suite) or `cargo test` (full suite)
as your default verification step — it wastes minutes and defeats the purpose
of the targeted script. Use `just test-pr` instead.
