# 02 — Architecture

## The governing idea

Architectural difficulty is roughly:

```
state × time × host × concurrency × consumers
```

A pure batch transformation scores near zero on all five and is easy to keep clean. A live,
stateful, concurrent UI attached to a host scores high on all five simultaneously.

**The strategy is therefore to shrink the hard part.** Five of the ten shipped crates are pure
pipelines with no state, no time and no IO — they get their cleanliness structurally. All
of the genuine architectural risk is concentrated in one crate (`runtime`), whose entire job
is to be the single place where state, time and concurrency are permitted to exist.

This is the specific failure the plan exists to avoid: in the Neovim plugin, state × time ×
host was smeared across all 79 source files, so every file was working on the hardest
problem class at once. See [Decisions §D1](05-decisions.md#d1--why-a-rewrite-rather-than-a-port).

## Workspace layout

Crates are created by the milestone that needs them, not up front: an empty crate
constrains nothing, since there is no code in it to violate a rule. What follows is the
intended set. `cargo xtask lint-arch` enforces the forbidden edges as each crate
appears, and reports which rules are still waiting on their crate so that none can
quietly stay dead.

```
codediff/
├── Cargo.toml                [workspace]
├── crates/
│   ├── vscode-diff-sys/      raw FFI + cc build of the C engine
│   ├── diff-types/           what a diff *is* — no deps, no C          pure
│   ├── file-types/           what a *file* is — named by every layer   pure
│   ├── vscode-diff/          safe wrapper → LinesDiff                    pure
│   ├── line-index/           where each character of a line sits      pure
│   ├── syntax/               text → normalized syntactic spans        pure
│   ├── align/                Alignment · rows · hunks · spans        pure
│   ├── vcs/                  git today, jj tomorrow
│   ├── runtime/              events · commands · effects · watcher
│   ├── ui/                   ratatui rendering + input
│   ├── codediff/             binary · composition root
│   └── fixtures/             dev-only: builds test repositories, emits a manifest
├── xtask/                    lint, sync, verify and generate tasks (not a build system)
├── vendor/libvscode-diff/    C source, copied from a pinned upstream tag
└── docs/
```

### What each crate is for

| crate | contains | purity |
|---|---|---|
| `vscode-diff-sys` | `build.rs` invoking `cc`; `#[repr(C)]` structs and `extern "C"` declarations, 1:1 with the C API | unsafe, ~150 lines |
| `diff-types` | the six structs a diff is made of: `LinesDiff`, `LineRange`, `DetailedLineRangeMapping`, `RangeMapping`, `MovedText`, `CharRange`. **No dependencies and no build script**, so everything downstream can name a diff without inheriting a C toolchain | pure |
| `file-types` | what a file under review is: `RepoPath` (both spellings, one constructor), `File` (a version on each side, either absent), `FileContent`, `DiffVersion`. **No dependencies**, so `vcs`, the pipeline and `ui` can all name it and a file's identity is converted at no boundary ([D28](05-decisions.md#d28)) | pure |
| `vscode-diff` | `compute(&[&str], &[&str], Options) -> LinesDiff` returning owned Rust types; eager conversion, frees C memory immediately. Re-exports `diff-types` so one dependency suffices | pure |
| `line-index` | where each character of a line sits: UTF-16 ↔ byte ↔ char ↔ grapheme ↔ cell, display width, tab expansion, cell-range slicing | pure |
| `syntax` | language detection; text → `SpanSet` of normalized `Class` values. The only crate that may name a syntax engine | pure |
| `align` | pairing lines from a `LinesDiff` plus two texts; fillers; hunks; inner-change byte ranges; unchanged regions | pure |
| `vcs` | `VcsBackend` trait, git subprocess implementation, blob reading, rev resolution | IO |
| `runtime` | `Event`, `Command`, `update`, effect runner, file watcher, request generations | IO + state + concurrency |
| `ui` | terminal lifecycle, layout, panes, widgets, input state machine, theme | state |
| `codediff` | argument parsing, config loading, backend construction, wiring | composition root |
| `fixtures` | builds git repositories in known states and emits a manifest; **no workspace dependencies**, so `vcs` and e2e tests can dev-depend on it without forming a cycle | dev-only |

Naming rules: crates are named after **the thing they contain**, never after a layer.
`core`, `common`, `utils` and `model` are banned — a crate name should state an admission
criterion that can be applied in review. "Does this belong in `align`?" has an answer;
"does this belong in `core`?" does not.

## Dependency graph

Strictly acyclic. Enforced by cargo — a violation is a compile error, not a review comment.

```
codediff ──> ui ──────> runtime ──> vcs ──────> line-index
              │            │  └────> syntax ───> line-index
              │            └──> align ────────> line-index
              │            │         └────────> diff-types
              └──> align, line-index, syntax

vscode-diff ──> diff-types                 the engine, named only by the
     └────────> vscode-diff-sys ──> C      composition root and by tests
```

Note where the C stops. `align` names a diff through `diff-types`, which has no
dependencies and no build script, so a clean `cargo build -p align` never invokes `cc` —
0.7s rather than 4.2s. The engine is reached only by `codediff`, which computes the diff,
and by `align`'s own tests, which use real engine output as an oracle. A dev-dependency
does not propagate, so that costs consumers nothing.

### Edges that must never exist

| forbidden edge | why |
|---|---|
| `ui` → `vcs` | prevents the renderer shelling out to git — the exact failure that produced a 674-line `explorer/render.lua` |
| `ui` → `vscode-diff` | rendering must consume model types, never compute diffs |
| `align` → `vscode-diff` **in what ships** | pairing is handed a diff, it does not compute one; the edge would drag a C toolchain into a pure crate. Allowed as a dev-dependency, since the tests use the engine as an oracle |
| `align`, `diff-types` → `vscode-diff-sys` | the model must never touch the FFI layer |
| **anything → a syntax engine, except `crates/syntax/src/engine/`** | keeps the engine swappable; `syntect::` or `tree_sitter::` appearing anywhere else fails CI |
| anything → `codediff` | the composition root is a leaf; nothing depends on it |
| `align`/`line-index`/`syntax`/`vscode-diff`/`diff-types` → anything with IO | keeps the pure core pure |

## Hard rules

| rule | rationale | enforcement |
|---|---|---|
| `align`, `line-index`, `vscode-diff`, `diff-types` perform no IO — no `std::fs`, no `std::process`, no sockets, no clock | pure core is trivially testable and cannot rot | CI lint + absent dependencies |
| the pure model builds without a C toolchain | `align` is proptest-tested and must stay cheap to build and portable; it also keeps the door open to a second engine, such as a pure-Rust fallback or a WASM target where `cc` cannot run | `cargo xtask lint-arch` refuses `align → vscode-diff` in `[dependencies]` while allowing it in `[dev-dependencies]` |
| no cyclic crate dependencies | the single mechanism that prevented the plugin's decay | cargo |
| soft cap 300 lines/file, hard cap 500 | forces splitting before a file becomes a junk drawer | `cargo xtask lint-size` |
| private by default; `pub(crate)` is the escalation; `pub` is deliberate | shrinks blast radius, keeps API surface countable | clippy + review |
| split modules by **noun** (a type owns its logic), never by verb | verb-splitting is what created the `actions`/`render`/`refresh` triplets and forced a global | review |
| `unsafe` is permitted only in `vscode-diff-sys` and in `vscode-diff`'s `convert` module | ~40 lines of reviewable unsafe outside the raw declarations | the other **seven** crates carry `#![forbid(unsafe_code)]`, which cannot be overridden from within; `vscode-diff` carries `#![deny]` with a single narrow `#[allow]`; CI asserts both |
| every async operation carries a `RequestId` | stale results are dropped structurally, not by revalidation | type system |

## Seams installed early

Each costs roughly twenty lines now and prevents a rewrite later. Implement the shape at the
milestone listed, even with a trivial body.

| seam | at | initial implementation |
|---|---|---|
| `Event` / `Command` / effect runner | S7 | three event types, synchronous |
| `VcsBackend` trait | S5 | git subprocess only |
| `Syntax` trait in `crates/syntax` | S7 | returns empty spans until S11 |
| `ContentSource { Blob, Worktree, Snapshot, Memory }` | S5 | only `Blob` and `Worktree` used |
| `hunk.review: Option<ReviewMark>` | S4 | always `None` |
| `RequestId(u64)` on every command | S7 | always 0 until S14 |
| `VisualRow` as an **enum** with room for non-diff rows | S7 | wraps `align::ViewLine` — but the enum must exist, or agent annotations require surgery |
| `AppState.docs: HashMap<DocId, Document>` | S7 | exactly one entry — but the map must exist, or multi-diff requires surgery |
| `AppState` is `serde`-serializable | S7 | free crash dumps and session replay |
| theme table, no hardcoded colors | S7 | one dark theme |

## Core data model

Built in S4. This is the keystone of the whole design.

```rust
/// Which file a line number refers to. Never `Left`/`Right`: those are places
/// on a screen, and inline view puts both on the same side.
pub enum Side { Original, Modified }

/// What one side shows on one row.
pub enum Slot { Line(u32), Filler }

pub enum ViewLineType { Unchanged, Modified, Deleted, Inserted }

pub struct Row { pub original: Slot, pub modified: Slot, pub kind: ViewLineType }

/// Borrows the diff and both files. Stores no rows and no text.
pub struct Alignment<'a> { /* LinesDiff, two line slices, hunks */ }

impl Alignment<'_> {
    pub fn rows(&self) -> impl Iterator<Item = Row>;   // computed, never stored
    pub fn row_count(&self) -> u32;
    pub fn spans(&self, side: Side, line: u32) -> Vec<Span>;   // byte ranges
    pub fn hunk_at(&self, side: Side, line: u32) -> Option<&Hunk>;
    pub fn moved(&self, side: Side, line: u32) -> Option<&Move>;
    pub fn unchanged(&self) -> Vec<Region>;
}
```

**This is VSCode's model.** Its `DiffState` is a thin wrapper over the engine result and its
alignment entries are line-range pairs; ours drops the two pixel fields it carries for line
wrapping and plugin-inserted boxes, neither of which a terminal has. See D18.

**`Alignment` owns its two files, and is therefore stored.** The pipeline builds one when a
file is opened and hands it over inside a `pipeline::file::Diff`, which a `SideBySide` or
`Inline` buffer then holds; drawing a frame only reads it.

It used to borrow, and that one fact propagated further than anything else in the project:
a borrowed alignment cannot outlive the function that builds it, so the pipeline's last
stage could not *return* its result and took a closure instead, and every type holding one
carried a lifetime down through `Session`, `View`, `Tab` and `Pane`. The cost of owning is
one copy of each file, once, at open. See [D27](05-decisions.md#d27).

**`ui` owns the viewport, and there is no scroll synchronisation.** A `Pane` holds one
`Viewport` — one row index — whatever its buffer draws with it, so the two columns of a diff
cannot drift. Position is on the pane rather than the buffer, so two panes over one buffer
scroll independently. Wrapping makes pairing depend on pane width, so the wrap-aware
alignment lives there too, not in `align`. See [D19](05-decisions.md#d19).

Consequences:

- **Nothing is stored per row.** A change of `original 2..3, modified 2..2` already says
  "one original line, no modified line", which *is* the filler. Materialising a row per line
  would mean a structure the size of the file, rebuilt on every save, that can disagree with
  the diff it came from. It grows with edits, not with file size: the `comprehensive_move`
  fixture is 404 lines and 7 changes.
- **Scrolling is one shared `scroll_offset`.** The plugin needed 536 lines
  (`scrollsync.lua` + `scroll.lua`) to fight Neovim's `topline`/`topfill` because fillers
  were virtual lines. Here a row index means the same thing on both sides.
- **Side-by-side, inline and compact are *projections*** — different walks of `view_lines()`, not
  three subsystems. The plugin spent 2,035 lines on what is one model and three functions.
  This is why the model names `Original`/`Modified` and not left and right. In `ui`
  each projection is a distinct buffer kind rather than a flag, because they emit different
  row sequences and a row index has to mean one thing ([D27](05-decisions.md#d27)).
- **A move is not a kind of row.** The engine reports a moved block as an ordinary deletion
  plus an ordinary insertion, and its move ranges need not agree with its change ranges — in
  `comprehensive_move` a move covers original 32..89 while a change covers 37..139. Moves are
  a lookup by line number. VSCode has the equivalent fields on `DiffMapping` commented out.
- `HunkId` is a **content hash** of the hunk, not an index. This is what makes review state
  and cursor position survive an agent rewriting the file underneath you, and it makes
  "what changed since I last looked" pure set arithmetic.

### Newtypes are load-bearing

Four distinct column concepts all look like `usize`, and the C engine reports UTF-16
columns because it mirrors VSCode. Confusing them produces invisible misalignment. `line-index`
therefore defines `ByteOff`, `CharIdx`, `Utf16Col`, `CellCol` as distinct types, and `align`
defines `LineNo` vs `LineIdx`. These conversions become compile errors rather than test
failures.

## Data flow

### Layers

```
                 ┌────────────────── codediff ──────────────────┐
                 │ clap · config · construct backends           │
                 │ the ONLY place that names concrete types     │
                 └────────────────────┬─────────────────────────┘
                        owns both     │
             ┌────────────────────────┴────────────────────────┐
             ▼                                                 ▼
   ┌───────── ui ──────────┐      Intent          ┌─────── runtime ───────┐
   │ viewport · cursor      │ ───────────────────▶ │ AppState              │
   │ focus · expanded set   │                      │ update(ev) → [Command]│
   │ input state machine    │ ◀─────────────────── │ effect runner         │
   │ render(&AppState)      │   &AppState (read)   │ watcher               │
   └───────────┬────────────┘                      └───────────┬───────────┘
               │ reads model types                             │ calls
               ▼                                               ▼
        align                                           vcs   ·   vscode-diff
               │                                                  │
               ▼                                                  ▼
     line-index · diff-types                              vscode-diff-sys → C
```

### The two loops

**Loop A — presentation. Fast, synchronous, never leaves `ui`.**

```
key `j` → input state machine → Motion::Down → viewport.cursor += 1 → render
```

Sub-millisecond. `runtime` is never notified. No channel, no allocation, no IO. This covers
roughly 90% of all interaction: scrolling, cursor, folds, focus, horizontal scroll.

**Loop B — data. Asynchronous, crosses threads.**

```
trigger → Intent → runtime::update → [Command] → effect runner (worker)
                                                        ↓
                                                Event ← channel
                                                        ↓
                                                runtime::update → AppState′ → render
```

10–500 ms. Only for work that needs git or a diff computation.

Keeping these separate is the entire performance story. In the plugin everything went
through one path because Neovim owned the loop.

### Startup

```
1  codediff  parse args → Invocation { repo, spec: WorktreeVsHead }
2  codediff  construct GitBackend (spawns long-lived `cat-file --batch` child)
3  codediff  construct Runtime + Display, enter run loop
4  runtime   → Command::LoadStatus { req: R1 }
5  effect    worker: vcs::status() → `git status --porcelain=v2 -z --no-optional-locks`
6  effect    → Event::Status(R1, Vec<StatusEntry>)          [channel]
7  ui        view: Tree::build(files, mode, flatten) → the rows
8  ui        render: the list populated, diff area empty
```

### Opening a file

```
1  ui        Enter → Intent::OpenFile(path)
2  runtime   state.diff_req = R2; → Command::LoadDiff { req: R2, path, base, head }
3  effect    worker: vcs::blob(base, path) + vcs::read_worktree(path)
4  effect    worker: vscode_diff::compute(...)             [rayon, CPU-bound]
5  effect    worker: align::Alignment::new(&diff, &left, &right)
6  effect    → Event::DiffReady(R2, doc)
7  runtime   if R2 != state.diff_req { drop }   ← stale results die structurally
8  runtime   state.docs.insert(id, doc)
9  ui        render: alignment.rows(DiffLayout::SideBySide) → rows → cells
```

### Refresh

```
watcher thread (notify)
  → raw fs events
  → debounce 100ms trailing + coalesce
  → classify:
       .git/{HEAD,index,refs/**,MERGE_HEAD,rebase-merge/**}  → Event::RepoChanged
       path ∈ current file set                                → Event::FileChanged(path)
       path ∉ set and not gitignored                          → Event::RepoChanged
  → runtime::update
       RepoChanged     → Command::LoadStatus
       FileChanged(p)  → Command::LoadDiff for p ONLY        ← targeted, not full rebuild
  → new data lands
  → ui re-resolves cursor by (path, HunkId); if that hunk is gone, nearest survivor
```

The plugin polls `git status` every 500 ms and rebuilds the entire tree and every diff. This
does the minimum work and preserves position by identity rather than by index.

## Threading

```
main thread     event loop + render      owns AppState and viewport
worker pool     git + diff computation   rayon
watcher thread  notify
```

One `crossbeam_channel::Sender<Event>` cloned to workers and the watcher; the main thread
selects on it plus terminal input.

**No async runtime.** No network IO, bounded concurrency, everything is either a blocking
subprocess or CPU-bound. Threads and channels are simpler to debug and strictly sufficient.
If an agent backend later needs `tokio`, it lives *inside* that crate with a bridge to the
sync channel — an implementation detail of one adapter, not a property of the application.

## Invariants

1. `ui` reads `&AppState` and never mutates it; it emits `Intent`s.
2. `runtime::update` is **pure**: `(state, event) → (state′, Vec<Command>)`. No IO, no
   spawning, no clock.
3. Only the effect runner performs IO or spawns anything.
4. Every `Command` carries a `RequestId`; results with a stale id are dropped in `update`.
5. `ui` owns presentation state, `runtime` owns domain state; the bridge between them
   is **stable identity** (`path`, `HunkId`) — never an index.
6. `vscode-diff`, `line-index`, `align` are pure and testable with no terminal, no
   repository and no threads.

Invariant 2 is the one that pays most: the entire application logic can be tested by feeding
a `Vec<Event>` and asserting on the resulting state and emitted commands.

## Known pressure points

Named in advance so they are cheap. Unnamed, each is a month-three refactor.

| # | pressure | mitigation |
|---|---|---|
| 1 | `AppState` grows into a god object | nest by domain (`state.list`, `state.diff`, `state.watch`); `update/` submodules may only touch their own sub-state; track its line count |
| 2 | the `runtime`/`ui` state boundary gets re-argued (is "expanded set" domain or presentation?) | expect to redraw this line once around S12; the rule is presentation, keyed by path |
| 3 | `line-index` gets pulled into rendering | strictly measurement and conversion, never drawing |
| 4 | syntax spans must composite with diff and inner-change spans | build the generic `SpanSet` compositor with priorities at S7, when only diff spans feed it |
| 5 | compact/fold projection shifts row indices and breaks cursor | viewport stores the cursor as a *domain* position and derives the visual row, never the reverse |
| 6 | effects sprout `unwrap` | effects return `Result`; errors become `Event::Error` shown in the status line; never panic, never silently swallow |

## Health line-index

Architecture decays invisibly. These make it visible. Wired into CI at S1.

- **crate graph acyclic** — free, cargo enforces
- **lines per crate, tracked over time** — alarm if `runtime` grows faster than the pure
  crates. That ratio *is* the health metric: the hard part must stay small.
- **`pub` item count per crate** — API surface growth is coupling growth, and it is countable
- **test dependencies of pure crates** — if `align` or `line-index` tests ever need a temp
  directory or a repository fixture, purity has leaked. This canary fires earliest.
