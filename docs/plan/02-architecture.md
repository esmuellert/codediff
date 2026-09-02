# 02 — Architecture

## The governing idea

Keep most of the code pure — no state, no IO, no threads. Concentrate the hard
parts (terminal, threads, git subprocesses) in as few crates as possible.

Six of the twelve crates perform no IO at all. They are testable with no
terminal, no repository and no threads.

## Workspace layout

```
codediff/
├── Cargo.toml                [workspace]
├── crates/
│   ├── vscode-diff-sys/      raw FFI + cc build of the C engine
│   ├── diff-types/           the six structs a diff is made of — no deps, no C
│   ├── file-types/           what a file under review is — no deps
│   ├── vscode-diff/          safe wrapper → LinesDiff
│   ├── line-index/           UTF-16 ↔ byte ↔ char ↔ cell, display width, tabs
│   ├── syntax/               language detection, text → coloured spans
│   ├── align/                pairing lines, fillers, hunks, inner-change spans
│   ├── explorer/             file list: tree, flat mode, grouping, filtering
│   ├── vcs/                  git subprocess: status, blob reads, cat-file
│   ├── pipeline/             wires vcs + vscode-diff + align for one file
│   ├── ui/                   terminal, input, rendering, theme, syntax worker
│   ├── codediff/             binary — argument parsing, wiring
│   └── fixtures/             dev-only: builds test git repositories
├── xtask/                    lint-arch, lint-size, fixture-repo, dev
├── libvscode-diff/           canonical C diff engine
└── docs/
```

### What each crate is for

| crate | what it does | purity |
|---|---|---|
| `vscode-diff-sys` | `build.rs` + `cc`; `#[repr(C)]` structs and `extern "C"` declarations | unsafe, ~150 lines |
| `diff-types` | `LinesDiff`, `LineRange`, `DetailedLineRangeMapping`, `RangeMapping`, `MovedText`, `CharRange` — no dependencies, no build script | pure |
| `file-types` | `RepoPath`, `ChangedFile`, `FileContent`, `DiffType` — no dependencies | pure |
| `vscode-diff` | `compute(&[&str], &[&str], Options) -> LinesDiff`; eager conversion, frees C memory immediately | pure (except `convert.rs`) |
| `line-index` | coordinate conversions: byte ↔ char ↔ UTF-16 ↔ cell; display width; tab expansion; cell-range slicing | pure |
| `syntax` | two engines (syntect + tree-sitter), normalized `Class` values, `SpanSet` | pure |
| `align` | `Alignment` from a `LinesDiff` + two texts; view lines, hunks, inner-change byte ranges | pure |
| `explorer` | file list model: tree with directories, flat mode, grouping by revision pair, filtering | pure |
| `vcs` | `git status`, `git cat-file --batch`, blob reading | IO |
| `pipeline` | four stages: read both sides → diff → align → return `DiffContent` | IO |
| `ui` | terminal lifecycle, input state machine, rendering, theme, syntax worker thread, file worker thread | IO + state |
| `codediff` | `main`, clap, debug subcommands, doctor | composition root |
| `fixtures` | builds deterministic git repos for tests; no workspace dependencies | dev-only |

Crate naming rule: named after what the crate contains, not after a layer.
`core`, `common`, `utils` and `model` are not used.

## Dependency graph

Acyclic. Cargo enforces this — a cycle is a compile error.

```
codediff ──> ui ──────────> pipeline ──> vcs
              │                │    └──> vscode-diff ──> diff-types
              │                │    └──> align ────────> diff-types
              │                │                   └──> file-types
              │                │                   └──> line-index
              │                └──> explorer ─────> file-types
              └──> align, explorer, line-index, syntax

vscode-diff ──> vscode-diff-sys ──> C
```

`align` names a diff through `diff-types`, which has no build script, so
`cargo build -p align` never invokes `cc`.

### Forbidden edges

Enforced by `cargo xtask lint-arch`.

| forbidden edge | why |
|---|---|
| `ui` → `vcs` | the renderer must not shell out to git |
| `ui` → `vscode-diff` | rendering consumes model types, never computes diffs |
| `align` → `vscode-diff` in `[dependencies]` | keeps the pure crate free of a C toolchain (allowed in `[dev-dependencies]` for oracle tests) |
| `align`, `diff-types` → `vscode-diff-sys` | the model must not touch the FFI layer |
| anything → a syntax engine, except `crates/syntax/src/engine/` | keeps the engine swappable |
| anything → `codediff` | the composition root is a leaf |
| pure crates → anything with IO | `align`, `explorer`, `line-index`, `syntax`, `vscode-diff`, `diff-types` |

## Rules

| rule | enforcement |
|---|---|
| pure crates perform no IO | `cargo xtask lint-arch` + absent dependencies |
| pure model builds without a C toolchain | `lint-arch` refuses `align → vscode-diff` in `[dependencies]` |
| no cyclic crate dependencies | cargo |
| soft cap 300 lines/file, hard cap 500 (non-test lines) | `cargo xtask lint-size` |
| `unsafe` only in `vscode-diff-sys` and `vscode-diff/src/convert.rs` | `#![forbid(unsafe_code)]` on the other crates; `lint-arch` checks |

## Core data model

`Alignment` (in `align`) pairs the two files line by line, given a `LinesDiff` from
the engine. It stores no rows and no text — everything is computed on demand from
the diff and the two files.

```rust
pub enum DiffVersion { Original, Modified }
pub enum Slot { Line(u32), Filler }
pub enum ViewLineType { Unchanged, Modified, Deleted, Inserted }
pub struct ViewLine { pub original: Slot, pub modified: Slot, pub kind: ViewLineType }
```

Key properties:

- Nothing is stored per row. A change of `original 2..3, modified 2..2` already
  says "one original line, no modified line" — that is the filler.
- `Original`/`Modified`, never left/right. Inline view draws both on one side.
- A move is not a type of view line — it is a lookup by line number.
- `HunkId` is a content hash, so cursor position survives a file rewrite.
- Newtypes (`ByteOff`, `CharIdx`, `Utf16Col`, `CellCol`, `LineNo`, `LineIdx`)
  turn coordinate confusion into compile errors.

## Data flow

### One loop

`ui::app::run` is the event loop. It does one thing per iteration:

1. Take whatever the file worker or syntax worker has finished.
2. Ask for anything newly on screen (file comparison, syntax colouring).
3. Wait for a key (or wake on a frame tick while work is outstanding).
4. Dispatch the key, draw.

Nothing in this loop computes a diff, touches git, or colours a line.

### Opening a file

```
ui            Enter on a list row
              → send the ChangedFile to the file worker thread
file worker   vcs::read both sides
              vscode_diff::compute
              align::Alignment::new
              → send DiffContent back
ui            install the buffer, draw
```

### Syntax colouring

```
ui            a new buffer is on screen
              → send the text + language to the syntax worker thread
syntax worker syntax::spans (syntect or tree-sitter)
              → send spans back
ui            merge spans into the store, redraw
```

### Threading

```
main thread       event loop + rendering     (owns Session, View, Theme)
file worker       one thread, blocking       (vcs + diff + align)
syntax worker     one thread, blocking       (syntect / tree-sitter)
```

No async runtime. No rayon. No watcher yet.
- **`pub` item count per crate** — API surface growth is coupling growth, and it is countable
- **test dependencies of pure crates** — if `align` or `line-index` tests ever need a temp
  directory or a repository fixture, purity has leaked. This canary fires earliest.
