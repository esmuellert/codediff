# 01 — Overview

## What we are building

A standalone, read-only terminal diff **reviewer**, aimed at the workflow where an LLM agent
edits a repository while a human reviews the result.

- **Read-only.** codediff never writes to the user's files. (The agent may write; codediff
  only observes.) It is not an editor.
- **Standalone.** No Neovim, no editor host. A single static binary.
- **Reuses the proven diff engine.** The C library `libvscode-diff` from
  [codediff.nvim](https://github.com/esmuellert/vscode-diff.nvim) — a port of VSCode's
  diffing algorithm — is compiled from source and statically linked.
- **Rendered with [ratatui](https://ratatui.rs).**

## MVP definition

**One scenario, fully finished.**

`codediff` with no arguments, run inside a git repository, shows the current working state:
an explorer panel listing changed files grouped into Changes / Staged Changes / Merge
Changes, and a side-by-side diff of the selected file with line- and character-level
highlighting.

The *scenario* is deliberately narrow. The *experience within it* is complete.

### In scope

| area | included |
|---|---|
| invocation | `codediff` (no args) — worktree and index vs `HEAD` |
| explorer | grouped file list, status letters, path collapsing, filtering, folds |
| diff view | side-by-side, line highlighting, character-level inner changes, fillers |
| syntax | syntax highlighting composited under diff highlighting |
| navigation | scroll, cursor, vim-style motions, hunk jumping, horizontal scroll |
| liveness | file watcher, targeted refresh, index/HEAD watching |
| correctness | renames, untracked, deleted, binary, CRLF, unicode, very long lines |
| chrome | help screen, status line, config file, `--help` / `--version` |

### Out of scope for MVP

Explicitly deferred, not forgotten:

- other invocations: `codediff <rev>`, `rev..rev`, `A...B`, path scoping
- file history / commit log view
- inline (single-pane) diff mode
- compact / folded-context mode
- conflict resolution UI (read-only 3-way *display* is a later candidate)
- directory comparison
- staging, unstaging, discarding — anything that mutates git
- review state, annotations, agent backend integration

### Post-MVP direction

The MVP is the substrate for the actual goal. Once it lands:

- review state per hunk, keyed by content hash so it survives agent rewrites
- "what changed since I last looked" — a diff of the diff
- agent backend integration (explanations, critique, requested changes)
- an MCP surface so the agent can query the same diff model the human reviews
- snapshot bases (`ContentSource::Snapshot`) and jj support for free auto-snapshots

The architecture is designed so these land additively. See
[Decisions §D12](05-decisions.md#d12--stress-testing-the-architecture-against-future-features).

## Effort

Roughly **9–12 weeks of focused solo work** to complete S1–S17.

The heavy milestones are S4 (the aligned model), S7 (first pixels), S11 (syntax
highlighting) and S12 (explorer). The bottleneck throughout is design decisions rather than
typing; agent assistance compresses the git and explorer work well and the model and text
work poorly.

## Non-goals

- Being a general-purpose editor, or gaining editing features later.
- Reimplementing the diff algorithm in Rust. The C engine is the upstream source of truth.
- Reimplementing Vim. The motion set is deliberately small (see
  [Decisions §D9](05-decisions.md#d9--a-deliberately-thin-motion-set)).
