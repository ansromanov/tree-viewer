# Usage & Keybindings

## Basic usage

```sh
mantis                    # view the current directory
mantis path/to/dir        # view a specific directory
mantis file.md            # open a single file directly
mantis --completions bash # generate shell completions (bash, zsh, fish, powershell)
mantis --print-man-page   # print the man page
mantis --language rust < file # force syntax highlighting for piped stdin
mantis --update           # self-update to the latest release
mantis --help             # print help (or -h)
mantis --version          # print version (or -V)
```

Press `?` or `F1` for in-app help, and `q` to quit.

## Pager mode

When no `<path>` is given and stdin is piped rather than a terminal, `mantis`
reads stdin instead of walking a directory:

```sh
git diff | mantis          # navigable side-by-side diff
kubectl logs pod | mantis  # highlighted, searchable log output
curl -s https://example.com/file.py | mantis --language python
```

Diff-shaped input (a `diff --git`/`diff --cc` header, an `@@ -` hunk header, or
a `--- `/`+++ ` file-marker pair) renders through the same side-by-side diff
view as git mode, starting in side-by-side layout regardless of the
`[git.diff] side_by_side` setting. Anything else is syntax-highlighted: pass
`--language <name>` (e.g. `--language rust`) to force it, otherwise mantis
sniffs the first line the way `syntect` detects shebangs and mode lines.

The tree pane collapses (there is no path driving the view) and focus starts
in the content pane, but the tree is still there — drag the splitter or
press `Tab` to browse the working directory alongside the piped content.
Keyboard input keeps working normally even though stdin is consumed by the
piped data: mantis reads keys from the controlling terminal (Unix: `/dev/tty`,
Windows: `CONIN$`) instead, the same trick `less` uses. Input is read to EOF
before the UI starts, so very large piped input delays the first frame rather
than streaming incrementally.

## Terminal compatibility

The main panels require at least 80 columns to remain usable. At smaller sizes,
mantis shows a resize message instead of rendering clipped tree and content panes.

### Keyboard enhancement

Every default binding uses plain `Ctrl+letter`, a bare/Shift letter, or a
named key — combinations that work identically on every terminal and OS.
Ctrl+Shift combinations are deliberately not used: kitty reserves
`ctrl+shift` for its own shortcuts (`kitty_mod`), Windows Terminal binds
`Ctrl+Shift+P`/`Ctrl+Shift+F` itself, and legacy terminals (macOS
Terminal.app, plain xterm, many SSH setups) can't even distinguish them from
plain `Ctrl`. Modifier bindings are matched case-insensitively, so CapsLock
or a stray Shift never breaks a shortcut.

On terminals with the kitty keyboard protocol (CSI-u), mantis additionally
matches bindings by physical key position, so shortcuts work on non-Latin
keyboard layouts.

| Terminal | Keyboard enhancement (layout independence) | Full mouse support |
|---|---|---|
| kitty | ✓ Full | ✓ |
| WezTerm | ✓ Full | ✓ |
| Ghostty | ✓ Full | ✓ |
| Alacritty 0.15+ | ✓ Full | ✓ |
| Windows Terminal | ✓ Full | ✓ |
| iTerm2 | ✓ Partial¹ | ✓ |
| macOS Terminal.app | ✗ | ✓ |
| xterm (plain) | ✗ | Partial² |
| Most SSH clients | ✗ | Depends on client |
| tmux (inside any terminal) | ✗ | ✗ |

¹ iTerm2 supports CSI-u (disambiguation + event types) but may not report
alternate keys for all keyboard layouts.

² xterm supports mouse events but the generic mouse protocol (SGR 1006) lacks
drag and release tracking. Enable `xterm-mouse` in tmux or `XTerm*decTerminalID:
> 280` for full SGR mouse.

On terminals without keyboard enhancement, mantis shows a one-time notice and
notes the limitation in the in-app help (`?` → Getting started).

The best terminals for full mantis keyboard support are **kitty, WezTerm,
Ghostty, Alacritty 0.15+**, and **Windows Terminal**.

## Session persistence

`mantis` automatically remembers your workspace state across restarts:
expanded directories, the last open file, scroll position, and git mode.
State is cached outside the project tree (`~/.local/state/mantis/`
or `%APPDATA%\mantis\`) so it survives re-clones and never writes
dotfiles into the repository. Each workspace root gets its own file under
the `sessions/` subdirectory. To reset the session for a directory, quit
and delete its file from the `sessions/` subdirectory in the state directory.

> 💡 **Can't remember a key?** Press `?` or `F1` for the help overlay, or `Ctrl+P`
> to open the command palette and search for an action by name — it shows you
> the shortcut too. New to `mantis`? Start with the [Quick Start](quickstart.md).

Bindings are editor-style (VS Code / Sublime conventions) and fully remappable
— see [Keybindings](configuration.md#keybindings) for the complete list, the
macOS (`Cmd`) variants, and the `tree:`/`content:` scoping syntax. The tables
below cover the shipped defaults; single letters (`q`, `p`, `t`, …) only work
while the **tree** panel is focused — the content pane's letter keyspace is
kept free, apart from the vim motions below, for future editing features. Any
action not listed with a content-pane key is still reachable from the command
palette (`Ctrl+P`).

## Global

These work no matter which panel is focused.

| Key                    | Action                  |
| ---------------------- | ----------------------- |
| `Ctrl+c`, `q` (tree)   | Quit                    |
| `F1`, `?`              | Toggle help             |
| `Ctrl+P`               | Command palette (fuzzy-find any action) |
| `Tab`                  | Switch panel            |
| `/`                    | Tree filter (tree) / in-file search (content) |
| `Ctrl+T`               | Global fuzzy file-name picker |
| `Ctrl+F`, `f` (tree)   | Content (full-text) search |
| `Ctrl+r`, `F5`, `r` (tree) | Reload tree         |
| `Ctrl+e`, `e` (tree)   | Open current file in `$EDITOR` |
| `y` (tree)             | Copy absolute path to clipboard |
| `Y` (tree)             | Copy path relative to tree root to clipboard |
| `y` (content)          | Copy current line (or selection if any) to clipboard |
| `Y` (content)          | Copy entire file content to clipboard |
| `.` (tree)             | Toggle hidden files     |
| `H` (tree)             | Git history of current file |
| `Ctrl+O`               | Recent files (jump to a recently opened file) |
| `p` (tree)             | Plugin palette (enable/disable plugins) |
| `Ctrl+g`               | Go to line              |
| `Ctrl+b`               | Toggle full-file blame (dedicated pane replacing the tree) |
| `B` (content)          | Toggle single-line blame bar for the active line |
| `t` (tree)             | Theme picker            |
| `Ctrl+D`               | Toggle git mode (changed files + diffs; the pickers above scope to changed files) |
| `F` (tree)             | Toggle flat / tree view in git mode |

## Tree panel

| Key                  | Action                       |
| -------------------- | ---------------------------- |
| `Up`/`k`, `Down`/`j` | Move selection               |
| `Enter`/`Right`/`l`  | Expand directory / open file |
| `Left`/`h`           | Collapse directory / go up   |
| `Backspace`          | Go up one directory (stops at the directory mantis was launched from) |
| `-`/`=`              | Collapse all / expand all    |
| `g`/`Home`, `G`/`End` | Jump to first / last entry  |
| `B`                  | Toggle bookmark on the open file |
| `b`                  | Open the bookmarks picker    |

Bookmarks are stored per workspace and restored across sessions. Use the
bookmarks picker to fuzzy-filter the pinned files, then press `Enter` to open
one. The default bindings are tree-scoped; you can change them in
`mantis.toml`.

## Content panel

The content pane has a **line cursor** (visible as a highlighted full-width row). Use `Up`/`Down` to move it, then press `B` to show a single-line blame bar for the highlighted line.

When full-file blame is toggled on (`Ctrl+b`), the tree panel is replaced by a dedicated blame pane listing every line's commit hash, author, date, and subject, kept in sync with the content cursor. Clicking a row in the blame pane jumps the cursor there and opens the single-line blame bar.

| Key            | Action                       |
| -------------- | ----------------------------- |
| `Up`/`k`, `Down`/`j` | Scroll / move line cursor |
| `PageUp`/`PageDown`  | Page up / down         |
| `Ctrl+Home`/`g`, `Ctrl+End`/`G` | Jump to top / bottom |
| `Left`/`Right` | Horizontal scroll (when wrap off) |
| `Home`/`0`     | Reset horizontal scroll      |
| `Space`        | Toggle fold at cursor        |
| `Ctrl+g`       | Go to line                   |
| `B`            | Blame the active line        |
| `/`            | In-file search               |
| `n`/`N`        | Next / previous hunk (in a diff) |
| `M`            | Toggle raw/rendered markdown (provided by markdown plugin) |

Word wrap, line numbers, JSON pretty-print, CSV/TSV table view, side-by-side diff, and the
staged/unstaged diff cycle have no default content-pane key — use the command
palette (`Ctrl+P`) or bind one yourself in `mantis.toml`.

### CSV and TSV table view

`.csv` and `.tsv` files are parsed (supporting RFC 4180 quotes and escaped characters) and rendered as aligned tables with Unicode box-drawing borders and headers. Like JSON pretty-printing, table formatting respects `prettify_size_limit` (files exceeding the limit show as raw text). You can toggle between table view and raw text using the command palette (`Ctrl+P` → "Toggle CSV/TSV table view") or by binding `toggle_table_view` in `mantis.toml`. Wide tables can be scrolled horizontally with `Left`/`Right`.

### Rendered plugin content and line numbers

`mantis` has no built-in markdown renderer; install and enable the `markdown` plugin (`p` in-app, or `[plugins.markdown]` in `mantis.toml`) for rendered Markdown. When a plugin renders a file's content, line numbers are hidden in the gutter. This is by design: rendered content collapses blank lines, strips code fences, and restructures formatting, so rendered-line numbers don't correspond to source-file line numbers.

When the markdown plugin is active and rendering a file, you can press `M` (or run "Toggle markdown render (markdown plugin)" from the command palette) to toggle between the raw file content and the rendered view.

## Git features

### Tree colors

Files and folders in the tree are colored by their git status:

| Color  | Meaning |
| ------ | ------- |
| Green  | New / untracked |
| Yellow | Modified |
| Red    | Deleted |
| Gray   | Ignored |

A directory takes the color of the most significant change inside it.

### Status bar

The status bar shows a git summary when inside a repository:

```
[branch  +ahead -behind  N changed]
```

### Git mode and diff navigation

| Key                 | Action |
| ------------------- | ------ |
| `Ctrl+D`             | Toggle git mode — show only changed files; opening a file shows its diff |
| `F` (tree)           | Toggle flat list / nested tree (git mode only) |
| `n` / `N`            | Jump to next / previous change hunk |
| `B` (content)        | Blame the current line: hash, author, date, summary |
| `H` (tree)           | File history — pick a commit to view its diff |

Side-by-side diff and the staged/unstaged diff cycle have no default key —
use the command palette (`Ctrl+P`) or bind one in `mantis.toml`.

## Search popup

Three search entry points cover different needs:

- **`Ctrl+T`** — global fuzzy file-name picker. Opens the same file-name search
  from either panel, regardless of focus. Use this when you want to jump to any
  file in the project by name.
- **`/`** — context-sensitive: in the tree panel it filters file names inline;
  in the content panel (with a file open) it opens the in-file search bar;
  otherwise it falls back to the file-name picker.
- **`Ctrl+F`** (or `f` in the tree panel) — fuzzy content search across
  all files (or changed files in git mode).

Open any search popup and just start typing to filter.
In git mode (`Ctrl+D`), searches are automatically scoped to only the
changed files — the popup title shows "(changed files)" to make this visible.

| Key       | Action                          |
| --------- | ------------------------------- |
| *(type)*  | Filter results                  |
| `Up`/`Down` | Navigate results              |
| `Tab`     | Switch files / content mode     |
| `Enter`   | Open selected result            |
| `Esc`     | Close search                    |
| `Ctrl+A` | Toggle case-sensitive matching (`[Aa]`) |
| `Ctrl+W` | Toggle whole-word matching (`[\b]`) |
| `Ctrl+R` | Toggle regular-expression matching (`[.*]`) |

The toggles apply to content search (`f`) and the in-file search bar (`/`);
the active options are shown as highlighted `[Aa] [\b] [.*]` indicators.

## Command palette

Press `Ctrl+P` to open a searchable list of **every** action, each shown
next to its current keybinding. Type to fuzzy-filter (e.g. "blame", "theme",
"json"), navigate with `Up`/`Down`, and press `Enter` to run the highlighted
command. It's the fastest way to discover what `mantis` can do without
memorizing keys.

Commands that don't apply to the current state (e.g. "Toggle JSON
pretty-print" with no JSON file open, or "Toggle blame" outside a git repo)
are shown dimmed with the reason in place of their description. Selecting
one anyway sets a status-bar message explaining why it didn't run, instead
of silently doing nothing.

### Prefix routing

The palette is also a unified quick-open: type one of these characters as
the **first** character of the query to switch what it searches:

| Prefix | Mode                                        |
|--------|---------------------------------------------|
| (none) | Commands (the default list above)           |
| `>`    | Commands (explicit alias)                   |
| `/`    | File search (fuzzy file names)              |
| `#`    | Content search (grep across files)          |
| `:`    | Go to line (`42`, `+5`, `-3`)               |

In the file/content modes, `Tab` toggles between the two (just like the
standalone search overlay), and `Enter` opens the selected result. Backspace
on an empty routed query — or `Esc` — returns to the commands list; a second
`Esc` closes the palette.

## Reporting a bug

Run **"Report a bug (save diagnostics locally)"** from the command palette to
open an interactive modal where you can write a description and preview the
collected diagnostics. Submitting the modal (`Ctrl+S` or `Ctrl+Enter`) saves the
report locally, attempts to open the GitHub issue creator in your browser
prefilled with the report, and falls back to copying the report to your clipboard
if needed. See [Telemetry & Bug Reports](telemetry.md) for exactly what the
report contains.

## Git mode history

`H` (while the tree is focused) opens the file's git history in both normal
and git mode. The diff of a selected commit stays on screen and won't be
replaced by live file-watcher updates. Press `Esc` or reload (`Ctrl+r`/`F5`)
to return to the current file (or the working-tree diff in git mode).

## Open in your editor

Press `Ctrl+e` (or `e` while the tree is focused) with a file open to launch
it in your editor. `mantis` uses `$VISUAL`, then `$EDITOR`, preferring `nano`
over `vim` when neither is set. The TUI suspends while the editor runs and
resumes when you exit; the file is reloaded afterwards so you see your changes.

> 💡 `$EDITOR` can include arguments — e.g. `export EDITOR="code --wait"` opens
> the file in VS Code and waits for you to close the tab before returning.

To override the editor without setting environment variables, add to
`mantis.toml`:

```toml
[general]
editor = "code --wait"
```

When `$VISUAL` and `$EDITOR` are both unset and no config override exists,
`mantis` probes for `nano`, then `vim`, and shows a status-bar hint on
return so you know how to change it.

## Status bar

The status bar at the bottom of the screen shows context-sensitive information
about the open file:

- **`Ln N`** — the active (highlighted) line number, 1-indexed.
- **`[Language]`** — the detected syntax name from syntect (e.g. `[Rust]`,
  `[Python]`, `[TOML]`). Hidden when the file type is not recognised or when
  viewing a diff.
- **Scroll percentage** — how far through the file the content pane is
  scrolled.
- **Encoding and line endings** — shown when `I` (file info) is toggled on.

## Code folding

Press `Space` to fold or unfold the block at the cursor. A fold gutter appears
in the content pane when foldable regions are detected, and the status bar shows
fold stats. Fold regions come from two sources: a built-in YAML indentation
detector, and language plugins that supply per-file-type regions over the
[plugin protocol](plugins.md). Plugin regions override the built-in output for
their file extension.

Note that folding for a mainstream language like Rust (`.rs`) or Go (`.go`) can be
provided by a language provider plugin — the bundled `rust` and `go` plugins register
this way. You must explicitly enable such plugins in `mantis.toml` (or via the plugin
manager popup) for folding to work on those files.

Objects and arrays in `.json` files can be folded the same way, via the bundled
`json` plugin (also opt-in). It folds against the pretty-printed view below, so
regions line up whether or not the file was originally minified.

Terraform / HCL files (`.tf`, `.tfvars`, `.hcl`) fold via the bundled
`terraform` language provider plugin, which detects HCL blocks while ignoring
`#`/`//`/`/* */` comments, quoted strings, and heredocs.

INI-family files (`.ini`, `.service`, `.timer`, `.conf`, `.properties`, `.cfg`,
`.desktop`) fold by section. SQL files (`.sql`) fold multiline statements and
common procedural blocks. Both providers are bundled and enabled by default.

## JSON pretty-printing

Viewing a JSON file? Use the command palette (`Ctrl+P` → "Toggle JSON
pretty-print") to reformat it with indentation for easier reading, and again
to return to the raw text. Handy for minified `.json`. There's no default key
for this — bind `toggle_pretty_json` in `mantis.toml` if you want one.
JSON pretty-printing itself is always core, not a plugin concern — the `json`
plugin only adds fold regions on top of it.

## JSON Lines

Files ending in `.jsonl` or `.ndjson` are shown as one compact object per line.
Press `Space` on an object to expand it into formatted JSON in place; press
`Space` again to collapse it. Invalid or partially-written lines remain visible
as plain text, which keeps mixed structured logs readable.

## Secret masking

Credential-shaped files such as `.env`, `.pem`, `credentials`, and `kubeconfig`
are masked by default. Values use a fixed-width placeholder so their length is
not exposed. Use the command palette action `Toggle secret reveal` to reveal or
mask the current file for the session; the value is never persisted.

Set `content.mask_secrets = false` to disable this protection.

Use the command palette action `Open JSON query bar` to filter or project JSON
and JSONL. The supported subset includes `.a.b[0]`, `.items[]`, `{name, image}`
and `select(.level == "error")`. Invalid queries keep the last valid result;
press `Esc` to restore the unfiltered view.

## Log follow mode

When viewing log files (detected via `.log` extension or level/timestamp sniffing), mantis automatically enables log mode.
- Press `F` to toggle log follow mode.
- When follow mode is enabled and pinned (default), mantis auto-scrolls to the tail when new logs are appended to the file.
- Navigating upwards (e.g., `Up`/`k`, `PageUp`) will temporarily unpin follow mode to let you read earlier lines. Navigating to the bottom (e.g., `G`/`End`) will re-pin follow mode.
- Press `&` to open the filter bar. Typing a query will filter the visible log lines to only those containing the query.

## Mouse

- **Click** a tree row to select it — opens a file, or folds/unfolds a
  directory.
- **Double-click** a directory to make it the new tree root.
- **Click** a pane to focus it.
- **Scroll wheel** scrolls whichever pane is under the cursor.
- **Double-click** a breadcrumb segment to navigate to that directory.
- In the search and history popups, **single-click** selects an entry and
  **double-click** activates it.
- **Right-click** a tree row or the content pane to open a context menu at the
  cursor. Tree menus offer open, open in editor / default app, reveal in file
  manager, copy absolute/relative path, expand/collapse (directory), and
  expand/collapse all. Content menus offer copy selection/line/file, word wrap
  and raw-markdown toggles, and reveal-in-tree. Navigate with the mouse or
  `j/k`/arrows, activate with Enter or a left-click, and dismiss with `Esc` or
  a click anywhere else.

### Worktree switcher

When a repository has multiple linked worktrees, use the command palette and
choose `Open worktree switcher`. The picker shows each worktree's branch and
changed-file count; type to filter, then press Enter to switch the tree to that
worktree. The status bar shows the total worktree count when more than one is
available.
