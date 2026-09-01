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

### Who uses it

1. **A human running an acceptance checklist.** S5, S6, S12, S13, S15 and S16 all begin
   `cd /tmp/cdfix`. It is the ground truth for "did the explorer list the right files, in
   the right groups, with the right status letters?"
2. **`crates/vcs/tests/`** — the porcelain-v2 parser can only be tested against real
   repositories containing real renames, conflicts, CRLF and binary files.
3. **`tests/e2e/`** — replay scripts run the real binary against it and compare frames.
4. **CI** — runs 2 and 3 on every push.
5. **Regression capture** — when a bug appears, its triggering case is added to the
   fixture and becomes permanent. Over time the fixture grows into an inventory of
   everything that has ever broken.

### Where it lives

The generator is a crate of its own, `crates/fixtures/`, with **no workspace dependencies**.
`cargo xtask fixture-repo` is a thin CLI wrapper over it for human use, while `vcs` and
`tests/e2e/` take it as a `[dev-dependencies]` entry.

It must not live inside `xtask`: `xtask` depends on the workspace crates, so `vcs`'s tests
depending on `xtask` would form a cycle. Cargo tolerates dev-dependency cycles, but relying
on that is unnecessary cleverness for no gain.

Because `fixtures` emits a plain manifest rather than sharing `vcs`'s types, drift
protection comes from assertion rather than from a shared type: a test compares parsed
`StatusEntry` values against the manifest, so any divergence still fails the build.

### Contents

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

## 2. Why `xtask` rather than shell scripts

Rust has no built-in task runner — `cargo` knows only `build`, `test`, `run`, `bench` and
`doc`. There is no `cargo run-script`. The community convention is `cargo xtask`: a normal
binary crate in the workspace plus an alias in `.cargo/config.toml`.

```toml
[alias]
xtask = "run --package xtask --"
```

`cargo xtask verify-c` is then literally `cargo run -p xtask -- verify-c`. It is not a
framework and nothing needs installing.

**xtask is not a build system.** It never compiles anything — `cargo build` and `build.rs`
do that, including the vendored C. It holds only the chores cargo has no opinion about.

For comparison, codediff.nvim's `Makefile` (duplicated in full as `Makefile.win`) has
thirteen targets, of which nine are free here:

| plugin | ours |
|---|---|
| `make build` | `cargo build` — `build.rs` compiles the C |
| `make test` / `test-c` / `test-lua` | `cargo test` |
| `make lint` | `cargo clippy` |
| `make format` | `cargo fmt` |
| `make clean` | `cargo clean` |
| `make bump-*` | `cargo release` |
| `make help` | `cargo xtask` with no arguments |
| `scripts/build-vscode-diff.sh` | `build.rs` |
| `scripts/test_diff_comparison.sh` | `cargo xtask verify-oracle` |

Both makefiles collapse to zero files, because cargo is already cross-platform.

Three of our tasks — `lint-arch`, `lint-size`, `verify-c` — have **no plugin equivalent**,
because Make and Lua had no way to express them. That is the real point:

> **xtask is where the rules in this plan stop being prose and become build failures.**

Cargo enforces exactly one architectural rule for free (acyclic crate dependencies).
Everything else — `ui` must not reach `vcs`, pure crates must declare no IO, files stay
under the size cap, the vendored C must match its pinned tag — is project-specific, and
therefore has to be encoded somewhere. That somewhere is `xtask`.

Writing it in Rust rather than shell also means it is cross-platform without duplication,
type-checked, testable, and able to `use` the workspace crates. `lint-arch` in particular
must parse every `Cargo.toml` and walk the dependency graph: roughly 200 lines of brittle
`jq`/`awk` in shell, or 40 lines of Rust using `toml` and `cargo_metadata`.

## 3. Debug subcommands

Each exposes one layer's internals as plain text. These are **shipped and permanent**, not
scaffolding, and they do three jobs:

1. **Make headless milestones human-checkable.** S1–S6 have no UI; `debug align` is how a
   human sees whether the model is correct.
2. **Permanent debugging.** A bug report becomes "send me `codediff debug align` output".
3. **They keep the layering honest.** Every pure crate must be independently drivable from
   outside. If `debug align` cannot be written without pulling in git, the layering is
   already broken — so these commands are a continuous test of the architecture, not merely
   an aid.

They also close a loop: **`debug align`'s output format is the golden snapshot format.** The
human-readable artifact and the regression fixture are the same file.

| command | shows | first used by |
|---|---|---|
| `codediff doctor` | version, engine version, linkage, deps, config path | S1 |
| `codediff debug diff <a> <b>` | the raw `LinesDiff` — changes, inner changes, moves | S2 |
| `codediff debug line <f> [-v]` | per-character byte/utf16/column/width for the characters that diverge, and for control characters | S3 |
| `codediff debug align <a> <b> [-v]` | plain-text side-by-side with fillers, change markers and move notes; `-v` adds hunks, character spans and unchanged regions | S4 |
| `codediff debug status [dir] [-v]` | parsed status entries in the manifest's own format, so the two can be diffed | S5 |
| `codediff debug show <rev>:<path> [--raw]` | a file at a revision; `--raw` writes the exact bytes, for `cmp` against `git show` | S6 |
| `codediff debug diff-file <path> [-v]` | aligned diff of one file, worktree vs HEAD — the whole pipeline | S6 |
| `codediff debug events` | the event log of a session, replayable | S14 |

## 4. xtask commands

| command | asserts | arrives |
|---|---|---|
| `cargo xtask verify-c` | vendored C matches the pinned upstream tag; fails on drift | S1 |
| `cargo xtask sync-c --tag vX.Y.Z` | refreshes the vendored C and its oracle fixtures, rewrites `vendor/UPSTREAM.lock` | S1 |
| `cargo xtask lint-size` | no file exceeds the hard cap, counting non-test lines only | S1 |
| `cargo xtask lint-arch` | no forbidden crate edge; pure crates declare no IO dependencies; `forbid(unsafe_code)` present where required | S1 |
| `cargo xtask verify-oracle` | our diff output matches upstream `diff_tool` on every fixture | S2 |
| `cargo xtask verify-vscode [repo]` | VS Code and codediff render the same rows and highlight roles over real Git history | S2 |
| `cargo xtask fixture-repo <dir>` | builds the fixture repository, prints the manifest | S5 |
| `cargo xtask dev [dir] [args...]` | runs `codediff`, rebuilding and starting it again each time F5 exits it | S13 |
| `cargo xtask drift` | lines and `pub` counts per crate, for trend tracking | later |

`verify-vscode` selects highly revised files from real Git history. A pinned
`@vscode/test-web` build renders every pair in headless Playwright Chromium;
`codediff debug parity` renders the same pair through `SideBySide`. Row pairing,
fillers, line and gutter roles, character ranges, line-break fill and empty
markers are compared exactly. The web tools form a pnpm workspace: its catalog
pins `@vscode/test-web` and Playwright, while the extension package owns both
`engines.vscode` and the commit downloaded by the runner. Both renderers emit
the JSONL records defined by `xtask/src/verify_vscode/schema.json`. The command
stores `vscode.jsonl`, `codediff.jsonl`, and `difference.jsonl` under
`target/vscode-parity/mismatches/`. The verifier disables the computation
budget on both sides: a wall-clock cutoff is deliberately timing-dependent,
whereas the completed mappings and their rendering are deterministic. Word
wrapping and color decorators are disabled so unrelated editor widgets cannot
change diff coordinates. External pressure-test clones live in the ignored
`target/vscode-parity-repos/`; only their committed history is read.

`drift` is named for what it measures. It is not `health`, which would collide with
`codediff doctor` — that reports on the *user's environment*, while this reports on the
*codebase*.

## 5. Automated test layers

Three layers, each with an admission criterion that can be answered yes or no:

| layer | criterion | lives in |
|---|---|---|
| **unit** | drives one function or type; may see private items | `#[cfg(test)] mod tests` in the same file |
| **integration** | drives **one crate's public API** | `crates/<name>/tests/` |
| **e2e** | drives **the real binary**, against a real repository | `tests/e2e/` |

"Integration test" is Cargo's own term for anything under `tests/`, so this taxonomy matches
the toolchain rather than competing with it.

| test | layer | tool | speed | proves |
|---|---|---|---|---|
| per-function logic | unit | `cargo test` | µs | one function or type |
| `line-index` conversions | unit + property | `proptest` | ms | invariants over generated input |
| `align` golden output | integration | std, `UPDATE_GOLDEN=1` | ms | `debug align` output unchanged, and each column still reads back as its file |
| `ui` screens | integration | `insta` + `TestBackend` | ms | rendered cell grid unchanged |
| `vcs` against git | integration | `cargo test -p vcs` | ~100 ms | real git behaviour, via fixture repos |
| `pipeline` + `ui` integration | integration | `cargo test -p codediff` | ms | file worker and session behaviour |
| replay scripts | e2e | `cargo test --test e2e` | seconds | the real binary, fully wired |
| pty smoke | e2e | `cargo test --test pty` | seconds | raw mode, alt-screen, terminal restore |
| oracle | — | `cargo xtask verify-oracle` | seconds | parity with the upstream C tool |
| acceptance | — | `docs/acceptance/S##.md` | minutes | a human ticked every box |

### Integration tests are the load-bearing layer

`crates/codediff/tests/` drives the session with real files and asserts on
rendered screens — no live terminal, fully deterministic.

### e2e is deliberately thin

A handful of scripts, not a suite. Rather than driving a pseudo-terminal — flaky, and
awkward on Windows — the binary takes a scripted mode:

```
codediff --replay tests/e2e/scripts/open-and-navigate.txt --dump-frames out/
```

This runs the real event loop, real git and real rendering, but writes each frame as text
instead of to a terminal: deterministic, diffable, cross-platform, and it doubles as a
bug-report format a user can send us.

Then **two or three pty smoke tests only**, covering what `--replay` genuinely cannot: raw
mode, alt-screen entry and exit, and terminal restoration on both quit and panic.

The risk of thin automated e2e is under-testing the real wiring. That is covered by the
third leg — the [acceptance checklists](#6-acceptance-checklists), run by a human at every
milestone. The intended shape is heavy unit and integration coverage, thin automated e2e,
and a human acceptance gate, rather than the usual pyramid with a starved top.

### Where unit tests go, and why it matters

Unit tests live **at the bottom of the file they test**, in `#[cfg(test)] mod tests`:

```rust
// crates/align/src/hunk.rs
pub struct Hunk { /* ... */ }
fn merge_adjacent(/* ... */) { /* private */ }

#[cfg(test)]
mod tests {
    use super::*;
    #[test] fn merges_adjacent_hunks() { /* can call the private fn */ }
}
```

This is not a style preference. A `#[cfg(test)] mod` is a **child module**, so it can reach
private items. Tests placed in `crates/*/tests/` see only the public API, which pressures
you into marking things `pub` purely for testing — directly undermining the "private by
default" rule in [Architecture](02-architecture.md#hard-rules).

So: in-file tests for every file with non-trivial logic; `crates/*/tests/` reserved for
genuine public-API integration tests.

**Consequence for the size cap:** `cargo xtask lint-size` counts **non-test lines only**.
Otherwise the 300/500-line cap punishes writing tests, and the natural response is to move
tests out of the file to stay under it — defeating both rules at once.

### `align` invariants (property-tested)

1. Left `Cell::Text` entries, read in row order, reproduce the original file exactly.
2. Right `Cell::Text` entries, read in row order, reproduce the modified file exactly.
3. No row has `Filler` on both sides.
4. Every changed line belongs to exactly one hunk.
5. Hunks are ordered and non-overlapping.
6. `HunkId` is stable under changes elsewhere in the file.

### Screen snapshots are the bridge between automated and human review

`TestBackend` renders to an in-memory cell grid. Serialised as text and committed, every
screen state becomes a file. A UI change then shows up as a **text diff in the pull
request** — simultaneously a regression test and a human-reviewable artifact. This is the
thing a Neovim plugin cannot do, and it is why the automated suite here can carry far more
weight than the plugin's.

## 6. Acceptance checklists

Each milestone has `docs/acceptance/S##.md`: numbered steps, the exact command to run, the
exact expected observation, and a checkbox. A milestone is done when a human has run the
checklist and every box is ticked.

Checklists are committed. When a milestone's behaviour legitimately changes later, the
checklist is updated in the same change.

### A pty makes most of the "manual" terminal checks automatic

`TestBackend` renders frames but never touches a terminal, so it cannot see the one failure
that matters most: a program that exits leaving raw mode on and the cursor hidden, which
takes a `reset` typed blind to recover from.

`portable-pty` allocates a real terminal, runs the built binary on it, sends keystrokes and
reads back every byte written — including the escape sequences, which are only emitted when
stdout *is* a terminal. So `crates/codediff/tests/terminal.rs` can assert that the
alternate screen is entered and left the same number of times, that the cursor is shown
again, that a panic message lands **after** the restore rather than on the screen about to
be discarded, and that `Ctrl-Z` stops the process with the terminal handed back while
`SIGCONT` brings it back and repaints in full.

That last one found a real bug: `Terminal::clear` round-trips to the terminal to read the
cursor position back, and the reply can be swallowed by anything else reading the same
stream.

## 7. Manual TUI checks that cannot be automated

What is left after the pty covers the rest — all of it about how it *looks*, which is the
one thing no assertion settles:

- running over SSH in a 256-colour terminal degrades sensibly
- holding a motion key stays smooth and never blocks
- the colours are legible against the reader's actual terminal background
- wide characters, combining marks and tabs line up on a real font
