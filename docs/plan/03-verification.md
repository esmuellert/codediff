# 03 — Verification

Every milestone must be provable by a human in a few commands, with an unambiguous pass or
fail. Automated tests are necessary but are not the acceptance gate.

The difficulty is that the early milestones are libraries with nothing to look at. The
answer is that **every milestone ships a command that makes its internals human-readable.**

## 1. The fixture repository

```
cargo xtask fixture-repo /tmp/cdfix
```

Creates a deterministic git repository in a known messy state, and **prints its own
manifest** of exactly what it created.

Contents:

| category | fixture |
|---|---|
| unstaged modification | `src/changed.rs` |
| staged modification | `src/staged.rs` |
| staged + further unstaged edits | `src/both.rs` |
| new untracked file | `src/untracked.rs` |
| untracked directory | `newdir/` |
| deleted file | `src/deleted.rs` |
| renamed file | `src/old_name.rs` → `src/new_name.rs` |
| merge conflict | `src/conflict.rs` |
| binary file | `assets/logo.png` |
| CRLF line endings | `src/crlf.rs` |
| unicode: CJK, emoji ZWJ, combining accents | `src/unicode.rs` |
| tabs mixed with spaces | `src/tabs.rs` |
| very long lines (500+ chars) | `src/longlines.rs` |
| large file (5000 lines) | `src/big.rs` |
| whitespace-only change | `src/ws.rs` |
| moved block | `src/moved.rs` |
| file with no trailing newline | `src/nonewline.rs` |

Because the generator emits the manifest, most manual checks reduce to **diffing two
lists** rather than exercising judgement. The same manifest is the expected value for
automated assertions, so the manual and automated checks cannot disagree.

The fixture repository is regenerated from scratch on every invocation. It is never
committed.

## 2. Debug subcommands

Each exposes one layer's internals as plain text.

| command | shows | first used by |
|---|---|---|
| `codediff doctor` | version, engine version, linkage, runtime deps, config path | S1 |
| `codediff debug diff <a> <b>` | the raw `Diff` — changes, inner changes, moves | S2 |
| `codediff debug measure --file <f>` | per-line cell ruler and a byte/char/utf16/cell table | S3 |
| `codediff debug align <a> <b>` | plain-text side-by-side with fillers and change markers | S4 |
| `codediff debug status` | parsed status entries as a table | S5 |
| `codediff debug show <rev>:<path>` | blob content as read through the vcs layer | S6 |
| `codediff debug diff-file <path>` | aligned diff of one file, worktree vs HEAD | S6 |
| `codediff debug events` | the event log of a session, replayable | S14 |

These are permanent, not scaffolding. They are how the project is debugged for the rest of
its life, and they keep the pure crates honest by making them independently drivable.

## 3. xtask verification commands

| command | asserts |
|---|---|
| `cargo xtask verify-c` | vendored C matches the pinned upstream tag; fails on drift |
| `cargo xtask sync-c --tag vX.Y.Z` | refreshes the vendored C, rewrites `vendor/UPSTREAM.lock` |
| `cargo xtask verify-oracle` | our diff output matches upstream `diff_tool` on every test pair |
| `cargo xtask fixture-repo <dir>` | builds the fixture repository, prints the manifest |
| `cargo xtask lint-size` | no file exceeds the hard cap |
| `cargo xtask lint-arch` | no forbidden crate edge; pure crates declare no IO dependencies |
| `cargo xtask health` | lines and `pub` counts per crate, for trend tracking |

## 4. Automated test layers

| layer | tool | scope |
|---|---|---|
| unit | `cargo test` | per-crate logic |
| property | `proptest` | `metrics` conversions, `align` invariants |
| golden | `insta` | `debug align` output for curated fixture pairs |
| oracle | `xtask verify-oracle` | parity with the upstream C tool |
| integration | `cargo test -p vcs` | git operations against generated fixture repos |
| **screen snapshot** | `insta` + ratatui `TestBackend` | the rendered cell grid, as text |
| event-level | `cargo test -p runtime` | feed `Vec<Event>`, assert state and emitted commands |

### Screen snapshots are the bridge between automated and human review

`TestBackend` renders to an in-memory cell grid. Serialised as text and committed, every
screen state becomes a file. A UI change then shows up as a **text diff in the pull
request** — which is simultaneously a regression test and a human-reviewable artifact.
This is the thing a Neovim plugin cannot do, and it is why the automated suite here can
carry far more weight than the plugin's.

### `align` invariants (property-tested)

1. Left `Cell::Text` entries, read in row order, reproduce the original file exactly.
2. Right `Cell::Text` entries, read in row order, reproduce the modified file exactly.
3. No row has `Filler` on both sides.
4. Every changed line belongs to exactly one hunk.
5. Hunks are ordered and non-overlapping.
6. `HunkId` is stable under changes elsewhere in the file.

## 5. Acceptance checklists

Each milestone has `docs/acceptance/S##.md`: numbered steps, the exact command to run, the
exact expected observation, and a checkbox. A milestone is done when a human has run the
checklist and every box is ticked.

Checklists are committed. When a milestone's behaviour legitimately changes later, the
checklist is updated in the same change.

## 6. Manual TUI checks that cannot be automated

A short standing list, re-run at every TUI milestone:

- `q` exits and leaves the terminal usable — prompt intact, cursor visible, no alt-screen
  residue
- `codediff --self-panic` panics and **still** restores the terminal
- resizing the terminal during use never corrupts the display
- `Ctrl-Z` suspend and `fg` resume works
- running over SSH in a 256-colour terminal degrades sensibly
- holding a motion key stays smooth and never blocks
