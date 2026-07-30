# 05 — Decisions

A log of decisions made during design, with the options rejected and why. The purpose is to
avoid relitigating them. When a decision changes, edit it here and mark what superseded it.

---

## D1 — Why a rewrite rather than a port

**Decision.** Rewrite the frontend in Rust rather than translating the Lua.

**Evidence from `codediff.nvim` (20,634 lines of Lua).**

| measurement | value |
|---|---|
| files calling `vim.api` / `vim.fn` directly | 53 of 79 (67%) |
| `require` calls inside function bodies (cycle workarounds) | 19 in `explorer/render.lua` alone |
| modules `explorer/render.lua` depends on | 14, including `git`, `view`, `side_by_side`, `lifecycle` |
| parameters to `create_session` | 14 positional |
| `lifecycle/accessors.lua` | 645 lines, 37 getters and setters over a global table |
| directories repeating the `{init, keymaps, render, refresh, actions, nodes}` shape | 4 |

**Root causes, in order of consequence.**

1. **Split by verb, not by noun.** `explorer/` is ten files each representing a *phase* of
   the same thing. Every phase file must import every other, and state cannot live in any of
   them — so it goes into a global. *Verb-splitting does not merely cause coupling; it
   necessitates a global.*
2. **Lua rewards creating cycles.** `require` is lazy and cached, so a cycle "works" if you
   defer it into a function body. There is no failure signal, so the architecture degrades
   silently and monotonically.
3. **No type system.** A node is "group", "directory", "commit" or a file — but in
   `explorer/` file nodes carry no `type` field at all and file-ness is inferred from its
   *absence*, while `history/`, using the same tree implementation, sets `type = "file"`
   explicitly. Two subsystems sharing one tree **disagree about their own data contract**
   and nothing detects it.
4. **No module privacy.** Every `M.foo` is public, so the dependency graph accretes rather
   than being designed.
5. **The host is never abstracted.** 67% of files call `vim.api`, so domain logic and
   presentation live in the same functions. There is no layer that can be lifted out intact
   — which is precisely why this is a rewrite.
6. **State keyed by host identity.** Sessions keyed by tabpage, watchers by buffer number.
   When the host changes something underneath, state is orphaned — the source of both the
   cleanup complexity and the refresh races.

**Fairness.** This is a normal trajectory, not incompetence. Every decision is reasonable at
3,000 lines and a liability at 20,000. Lua and the plugin model actively push this way: no
types, no privacy, no acyclicity, `vim.api` always in scope, and a runtime where the cheap
workaround always works. The plugin is feature-rich, well tested, and ships a hand-ported
VSCode algorithm in C.

**Could Lua be fixed?** Roughly 80% — a require-graph cycle linter (~50 lines, the highest
value item by far), LuaLS in strict mode with `---@class` and `---@enum`, an import-boundary
linter, a host adapter, and a session object with a metatable. But all of it is opt-in and
all of it must be adopted *before* the damage. In Rust these are defaults.

---

## D2 — Reuse the C engine, compiled from source

**Decision.** Copy `libvscode-diff` source into `vendor/`, pinned to an upstream tag, and
compile it with the `cc` crate. Do **not** consume the prebuilt `.so` from releases.

**Why the prebuilt library was rejected.** A `.so` / `.dylib` / `.dll` is by definition a
*dynamic* library; it cannot be statically linked into a standalone executable. Measured on
the target machine:

| | prebuilt `.so` | compiled from source |
|---|---|---|
| files to ship | 3 (exe + `.so` + `libgomp`) | **1** |
| runtime deps | `libgomp.so.1`, `libpthread` + RPATH setup | libc, libm |
| `cargo install` works | ✗ | ✓ |
| cross-compilation | ✗ | ✓ |
| build cost | 0 | **2.1 s** |
| offline / air-gapped build | ✗ | ✓ |
| ABI drift | silent memory corruption | **compile error** |

The shipped `.so` declares `NEEDED libgomp.so.1` and only resolves because the plugin bundles
a copy of libgomp beside it — which is why `installer.lua` contains ~130 lines of ldconfig
probing, `$ORIGIN` RPATH and Nix workarounds. Compiling from source with OpenMP disabled
removes that entire problem.

The library is 12 C files, 7,538 lines, with no external dependencies (utf8proc is
vendored). It builds to a 446 KB static archive in **2.1 seconds**. There is no build cost
worth avoiding.

Disabling OpenMP loses intra-diff parallel refinement. This is acceptable and arguably
better: the explorer parallelises *across files* with rayon, which yields more than four
threads inside a single file's diff.

---

## D3 — Copy the C source, do not submodule (for now)

**Decision.** `vendor/libvscode-diff/` is a copy from a pinned upstream tag, refreshed by
`cargo xtask sync-c` and guarded by `cargo xtask verify-c` in CI.

**Context.** Git submodules are in fact the dominant pattern for `-sys` crates bundling C —
verified: `libgit2-sys` (rust-lang), `libz-sys` (rust-lang), `curl-sys`, `openssl-src`,
`zstd-sys` and `harfbuzz-sys` all use them. `libsqlite3-sys` is the exception, copying the
amalgamation.

**Why copy anyway, for now.** Submodule friction is real — `clone --recursive`, CI checkout
configuration, confusing errors for contributors — and the C changes rarely while the Rust
project will iterate fast. `verify-c` gives drift *detection* where a submodule gives drift
*prevention*; both are adequate, and detection costs less day to day.

**Reframing.** This is not vendoring. Vendoring means keeping a copy of someone else's code
you do not control. This C is first-party, shared between two first-party consumers.

**Superseded when.** The C stabilises or a third consumer appears. Then extract
`libvscode-diff` into its own repository and submodule it from both projects, so that
codediff.nvim and codediff become **peer consumers of one upstream** rather than one being a
satellite of the other. Publishing `vsdiff-sys` to crates.io is a later, optional step, and
is not a prerequisite for anything.

---

## D4 — Crate boundaries as the architectural firewall

**Decision.** Split the project into nine crates with a strictly acyclic dependency graph
declared in `Cargo.toml` **before any logic is written**.

**Rationale.** Rust modules within a crate may reference each other freely, so module
structure alone enforces nothing — exactly the situation Lua was in. Crates cannot form
cycles, and `pub(crate)` provides genuine package-private visibility, which TypeScript and
Lua both lack. Splitting is free: everything statically links into one binary, and
incremental builds get faster because only changed crates recompile.

The critical missing edge is `display → vcs`. Because that dependency is not declared, a
renderer that shells out to git is a compile error — preventing by construction the failure
that produced a 674-line `explorer/render.lua`.

---

## D5 — Crate naming

**Decision.** No `codediff-` prefix. Crates named after the thing they contain, never after
a layer.

```
vsdiff-sys  vsdiff  metrics  align  explorer  vcs  runtime  display  codediff
```

**Rationale.** `core`, `common`, `utils` and `model` are banned not on aesthetic grounds but
because they are **unfalsifiable**. "Does this belong in `align`?" has an answer. "Does this
belong in `core`?" is always yes. A crate name should state an admission criterion that can
be applied in code review.

`metrics` covers measurement and coordinate mapping — the established term for both.
`align` is named for its algorithm, not its data, because the alignment builder and the
projections are the valuable part. `runtime` states its criterion: things that exist only
while the application is running. `vcs` rather than `git` leaves room for jj and avoids the
crates.io name.

Registry uniqueness is irrelevant for path dependencies. If publishing ever happens, add
`package = "codediff-align"` to the dependency line — one line per dependent, at that time.

---

## D6 — `runtime` and `display` state split

**Decision.** `runtime` owns domain state; `display` owns presentation state. Events that
change *data* go to `runtime`; events that change *presentation* never leave `display`.

**Consequence.** `j`, `k`, scrolling, folds and focus are handled entirely within `display`
— synchronous, sub-millisecond, no channel. Only file selection, refresh and watcher events
reach `runtime`. This is the two-loop model, and it is the whole performance story.

**The follow-on problem** — preserving cursor and selection across a refresh — is solved by
keying presentation state to **stable domain identity** (`path`, `HunkId`) that the model
provides. `display` re-resolves its own position after any refresh without `runtime` ever
knowing what a cursor is.

**Expect** to redraw this line once, around S12, over questions like whether the explorer's
expanded-set is domain or presentation state. (It is presentation, keyed by path.)

---

## D7 — `HunkId` is a content hash

**Decision.** Hunks are identified by a hash of their content, not by index.

**Rationale.** Agents rewrite files constantly; line numbers move while hunk content often
does not. Content-hash identity buys three things at once:

1. cursor and selection survive a refresh
2. review state survives an agent rewriting the file
3. "what changed since I last looked" becomes pure set arithmetic —
   `new_hunks − old_hunks` is exactly the newly appeared hunks

The third was not the reason for the decision but falls out of it, which is good evidence
the decision is right.

---

## D8 — No async runtime

**Decision.** Threads and channels — `crossbeam-channel`, `rayon`, a watcher thread. No
`tokio`.

**Rationale.** No network IO, bounded concurrency, and every operation is either a blocking
subprocess or CPU-bound. An async runtime would add coloured functions and
`Send + 'static` constraints on model types for no benefit.

**When an agent backend arrives**, if its client needs `tokio`, the runtime lives *inside*
that crate with a bridge onto the sync channel. The async runtime becomes an implementation
detail of one adapter rather than a property of the application — a direct dividend of the
adapter shape.

---

## D9 — A deliberately thin motion set

**Decision.** `j k h l Ctrl-D Ctrl-U gg G ]c [c Tab Enter / n N q ?` and counts. Nothing
else.

**Rationale.** Cursor, viewport and motions are entirely net-new work that Neovim previously
supplied for free; it is 500–1,000 lines to do convincingly. Reimplementing Vim is an
unbounded commitment that contributes nothing to the core thesis. Additional motions can be
added on demand, from evidence.

---

## D10 — `git status --porcelain=v2 -z`

**Decision.** Porcelain v2 with NUL separators, not v1.

**Rationale.** The plugin hand-parses `old -> new` rename arrows and quoted paths from
porcelain v1, which is fragile with spaces, unicode and unusual filenames. v2 with `-z`
eliminates that entire class of bug. `--no-optional-locks` on every invocation — upstream
already learned this ("stop E211 noise and index.lock contention from diff views").

Blob reads go through a long-lived `git cat-file --batch` process rather than one
`git show` spawn per file.

---

## D11 — Syntax highlighting is in the MVP

**Decision.** Included, at S11, using `syntect` (via `two-face`) behind a `Highlighter`
trait.

**Rationale.** An earlier draft deferred syntax highlighting entirely, on the grounds that it
is the largest single net-new subsystem and contributes nothing to proving the core thesis.
That reasoning still holds for *ordering* but not for *scope*: the MVP is defined as one
narrow scenario experienced completely, and a diff reviewer without syntax highlighting is
not complete.

`syntect` over `tree-sitter` for MVP because it bundles cleanly into a single static binary
with no grammar curation. The trait allows tree-sitter to replace it without touching the
renderer. The `SpanSet` compositor with priorities is built at S7, before any syntax spans
exist, so that composition is never retrofitted.

---

## D12 — Stress-testing the architecture against future features

The architecture was tested against six planned agent-review features. Five require no
structural change; one forced a decision that is now made.

| test | outcome |
|---|---|
| connect to an agent backend (streaming, cancellable) | **clean** — new crate beside `vcs`, plus event variants. `RequestId` already handles cancellation |
| agent comments displayed inline against hunks | **required a decision**: `VisualRow` must be an enum with room for non-diff rows, and projections must take a context struct. Made at S4 |
| "what changed since I last looked" | **free** — falls out of `HunkId` being a content hash |
| MCP server so the agent queries the diff | **clean** — everything except `display` and `codediff` is already headless |
| base revision = "when the agent started" | **clean** — `ContentSource::Snapshot` reserved; free if the repo is jj-backed |
| agent writes files while you review | **clean** — read-only means *codediff* never writes, not that nothing changes; the watcher already covers it |

### Risks that would force a genuine rewrite

| risk | insurance taken now |
|---|---|
| review state becomes primary and diff secondary, making `align` the wrong centre | watch for annotations gaining more fields than hunks |
| multiple simultaneous diffs (three-way, tabs, comparing two revisions) | `AppState.docs: HashMap<DocId, AlignedDoc>` from the start, with one entry |
| a GUI or web frontend | already covered by the `display` split |
| crash recovery, session replay, server-side review | `AppState` is `serde`-serializable from the start |
| `runtime` becomes a god object | `update/` submodules touch only their own sub-state; line count tracked in CI |

---

## D13 — jj support is a feature, not a nicety

**Note, not yet a decision.** jj auto-snapshots the working copy on every operation, so its
operation log answers "what did the agent change since T" **for free** — the single feature
that would otherwise require building a content-addressed snapshot store. This is the
strongest argument for the `VcsBackend` trait existing from S5 rather than being retrofitted.

---

## Open questions

| # | question | needed by |
|---|---|---|
| 1 | three-state explorer or simple worktree-vs-HEAD? *(recommend three-state)* | S5 |
| 2 | `syntect` or `tree-sitter`? *(recommend syntect for MVP)* | S11 |
| 3 | inline mode in MVP? *(recommend no — a projection, ~2 days later)* | S7 |
| 4 | binary / symlink / mode-change / submodule presentation | S6 |
| 5 | licensing and `ATTRIBUTION.md` — the C is VSCode-derived and vendors utf8proc | S1 |
