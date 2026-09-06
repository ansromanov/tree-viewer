# .agent

Canonical home for agent (AI coding assistant) configuration and skills.

`.claude/` and `.opencode/` in the repo root are **symlinks to this directory**, so
Claude Code and opencode share one config + skills tree instead of drifting apart.
Each tool reads its own filenames here and ignores the rest:

| Path | Used by |
|---|---|
| `settings.json` | Claude Code (hooks, permissions) |
| `opencode.json` | opencode (if present) |
| `skills/` | both (e.g. `skills/<name>/SKILL.md`) |

Project conventions live in [`../AGENTS.md`](../AGENTS.md), which both tools read from
the repo root.

## Worktree requirement

Every agent and harness session must create and use a dedicated git worktree before
editing, building, testing, committing, or pushing. Create it from a fresh remote
default branch with `git worktree add .agent/worktrees/<branch-name> -b <branch-name>
origin/main`, then run all work from that directory. Remove the worktree after the
branch is merged or abandoned; keep the primary checkout untouched.

## Note on symlinks

Git stores these as symlinks (mode `120000`). On Windows, `git config core.symlinks`
must be `true` (and the clone done with Developer Mode or admin) for them to
materialize as links rather than plain text files. This repo's maintainer works on
macOS; Windows contributors who hit broken `.claude`/`.opencode` should recreate them
as links pointing at `.agent`.
