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

```
codediff/
├── Cargo.toml                [workspace]
├── crates/
│   ├── vscode-diff-sys/      raw FFI + cc build of the C engine
│   ├── vscode-diff/          safe wrapper → Diff                      pure
│   ├── metrics/              text measurement + coordinate mapping    pure
│   ├── syntax/               text → normalized syntactic spans        pure
│   ├── align/                AlignedDoc · rows · hunks · projections  pure
│   ├── explorer/             entries · grouping · tree · filter       pure
│   ├── vcs/                  git today, jj tomorrow
│   ├── runtime/              events · commands · effects · watcher
│   ├── display/              ratatui rendering + input
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
| `vscode-diff` | `compute(&[&str], &[&str], Options) -> Diff` returning owned Rust types; eager conversion, frees C memory immediately | pure |
| `metrics` | UTF-16 ↔ byte ↔ char ↔ grapheme ↔ cell, display width, tab expansion, cell-range slicing | pure |
| `syntax` | language detection; text → `SpanSet` of normalized `Class` values. The only crate that may name a syntax engine | pure |
| `align` | building an `AlignedDoc` from a `Diff` plus two texts; hunks; projections; navigation primitives | pure |
| `explorer` | status entries → grouped tree, path collapsing, gitignore-style filtering | pure |
| `vcs` | `VcsBackend` trait, git subprocess implementation, blob reading, rev resolution | IO |
| `runtime` | `Event`, `Command`, `update`, effect runner, file watcher, request generations | IO + state + concurrency |
| `display` | terminal lifecycle, layout, panes, widgets, input state machine, theme | state |
| `codediff` | argument parsing, config loading, backend construction, wiring | composition root |
| `fixtures` | builds git repositories in known states and emits a manifest; **no workspace dependencies**, so `vcs` and e2e tests can dev-depend on it without forming a cycle | dev-only |

Naming rules: crates are named after **the thing they contain**, never after a layer.
`core`, `common`, `utils` and `model` are banned — a crate name should state an admission
criterion that can be applied in review. "Does this belong in `align`?" has an answer;
"does this belong in `core`?" does not.

## Dependency graph

Strictly acyclic. Enforced by cargo — a violation is a compile error, not a review comment.

```
codediff ──> display ──> runtime ──> vcs ──────> metrics
              │            │  └────> syntax ───> metrics
              │            └──> align ────────> metrics
              │            │         └────────> vscode-diff ──> vscode-diff-sys
              │            └──> explorer
              └──> align, explorer, metrics, syntax
```

### Edges that must never exist

| forbidden edge | why |
|---|---|
| `display` → `vcs` | prevents the renderer shelling out to git — the exact failure that produced a 674-line `explorer/render.lua` |
| `display` → `vscode-diff` | rendering must consume model types, never compute diffs |
| **anything → a syntax engine, except `crates/syntax/src/engine/`** | keeps the engine swappable; `syntect::` or `tree_sitter::` appearing anywhere else fails CI |
| anything → `codediff` | the composition root is a leaf; nothing depends on it |
| `align`/`explorer`/`metrics`/`syntax`/`vscode-diff` → anything with IO | keeps the pure core pure |

## Hard rules

| rule | rationale | enforcement |
|---|---|---|
| `align`, `explorer`, `metrics`, `vscode-diff` perform no IO — no `std::fs`, no `std::process`, no sockets, no clock | pure core is trivially testable and cannot rot | CI lint + absent dependencies |
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
| `VisualRow` as an **enum** with room for non-diff rows | S4 | only diff variants — but the enum must exist, or agent annotations require surgery |
| `AppState.docs: HashMap<DocId, AlignedDoc>` | S7 | exactly one entry — but the map must exist, or multi-diff requires surgery |
| `AppState` is `serde`-serializable | S7 | free crash dumps and session replay |
| theme table, no hardcoded colors | S7 | one dark theme |

## Core data model

Built in S4. This is the keystone of the whole design.

```rust
/// 1-based line number, as the C engine reports.
pub struct LineNo(u32);
/// 0-based index into a Vec of lines.
pub struct LineIdx(u32);

pub enum Cell { Text { line: LineIdx }, Filler }

pub enum RowKind {
    Unchanged,
    Inserted,
    Deleted,
    Modified,
    MovedFrom(MoveId),
    MovedTo(MoveId),
}

pub struct Row { pub left: Cell, pub right: Cell, pub kind: RowKind }

pub struct AlignedDoc {
    pub rows:  Vec<Row>,
    pub hunks: Vec<Hunk>,
    pub hit_timeout: bool,
}
```

Consequences:

- **Scrolling is one shared `scroll_offset`.** The plugin needed 536 lines
  (`scrollsync.lua` + `scroll.lua`) to fight Neovim's `topline`/`topfill` because fillers
  were virtual lines. Here fillers are rows.
- **Side-by-side, inline and compact are *projections*** — functions
  `&AlignedDoc -> Vec<VisualRow>` — not three subsystems. The plugin spent 2,035 lines on
  what is one model and three functions.
- **Hunk navigation is a scan over `rows`.**
- `HunkId` is a **content hash** of the hunk, not an index. This is what makes review state
  and cursor position survive an agent rewriting the file underneath you, and it makes
  "what changed since I last looked" pure set arithmetic.

### Newtypes are load-bearing

Four distinct column concepts all look like `usize`, and the C engine reports UTF-16
columns because it mirrors VSCode. Confusing them produces invisible misalignment. `metrics`
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
   ┌─────── display ───────┐      Intent          ┌─────── runtime ───────┐
   │ viewport · cursor      │ ───────────────────▶ │ AppState              │
   │ focus · expanded set   │                      │ update(ev) → [Command]│
   │ input state machine    │ ◀─────────────────── │ effect runner         │
   │ render(&AppState)      │   &AppState (read)   │ watcher               │
   └───────────┬────────────┘                      └───────────┬───────────┘
               │ reads model types                             │ calls
               ▼                                               ▼
        align  ·  explorer                              vcs   ·   vscode-diff
               │                                                  │
               ▼                                                  ▼
            metrics                                        vscode-diff-sys → C
```

### The two loops

**Loop A — presentation. Fast, synchronous, never leaves `display`.**

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
7  runtime   update: explorer::build(entries, cfg) → AppState.tree
8  display   render: explorer populated, diff area empty
```

### Opening a file

```
1  display   Enter → Intent::OpenFile(path)
2  runtime   state.diff_req = R2; → Command::LoadDiff { req: R2, path, base, head }
3  effect    worker: vcs::blob(base, path) + vcs::read_worktree(path)
4  effect    worker: vscode_diff::compute(...)             [rayon, CPU-bound]
5  effect    worker: align::AlignedDoc::build(left, right, diff)
6  effect    → Event::DiffReady(R2, doc)
7  runtime   if R2 != state.diff_req { drop }   ← stale results die structurally
8  runtime   state.docs.insert(id, doc)
9  display   render: align::project::side_by_side(&doc, &ctx) → rows → cells
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
  → display re-resolves cursor by (path, HunkId); if that hunk is gone, nearest survivor
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

1. `display` reads `&AppState` and never mutates it; it emits `Intent`s.
2. `runtime::update` is **pure**: `(state, event) → (state′, Vec<Command>)`. No IO, no
   spawning, no clock.
3. Only the effect runner performs IO or spawns anything.
4. Every `Command` carries a `RequestId`; results with a stale id are dropped in `update`.
5. `display` owns presentation state, `runtime` owns domain state; the bridge between them
   is **stable identity** (`path`, `HunkId`) — never an index.
6. `vscode-diff`, `metrics`, `align`, `explorer` are pure and testable with no terminal, no
   repository and no threads.

Invariant 2 is the one that pays most: the entire application logic can be tested by feeding
a `Vec<Event>` and asserting on the resulting state and emitted commands.

## Known pressure points

Named in advance so they are cheap. Unnamed, each is a month-three refactor.

| # | pressure | mitigation |
|---|---|---|
| 1 | `AppState` grows into a god object | nest by domain (`state.explorer`, `state.diff`, `state.watch`); `update/` submodules may only touch their own sub-state; track its line count |
| 2 | the `runtime`/`display` state boundary gets re-argued (is "expanded set" domain or presentation?) | expect to redraw this line once around S12; the rule is presentation, keyed by path |
| 3 | `metrics` gets pulled into rendering | strictly measurement and conversion, never drawing |
| 4 | syntax spans must composite with diff and inner-change spans | build the generic `SpanSet` compositor with priorities at S7, when only diff spans feed it |
| 5 | compact/fold projection shifts row indices and breaks cursor | viewport stores the cursor as a *domain* position and derives the visual row, never the reverse |
| 6 | effects sprout `unwrap` | effects return `Result`; errors become `Event::Error` shown in the status line; never panic, never silently swallow |

## Health metrics

Architecture decays invisibly. These make it visible. Wired into CI at S1.

- **crate graph acyclic** — free, cargo enforces
- **lines per crate, tracked over time** — alarm if `runtime` grows faster than the pure
  crates. That ratio *is* the health metric: the hard part must stay small.
- **`pub` item count per crate** — API surface growth is coupling growth, and it is countable
- **test dependencies of pure crates** — if `align` or `metrics` tests ever need a temp
  directory or a repository fixture, purity has leaked. This canary fires earliest.
