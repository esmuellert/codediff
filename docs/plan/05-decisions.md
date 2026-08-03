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

**This failure mode is not hypothetical.** Two upstream user reports:

- **#48** — `GLIBC_2.38 not found (required by /lib/x86_64-linux-gnu/libgomp.so.1)`;
  installation fails outright on a snap-provided glibc.
- **#58** — `libgomp.so.1: cannot open shared object file`; a Nix-installed Neovim cannot
  see the system libgomp, requiring the user to patch `LD_LIBRARY_PATH` through Home
  Manager.

Upstream's own issue #482 draws the same conclusion: *"the libgomp saga (#48, #58) taught us
that dynamically linked C++ runtime deps are the failure mode. Static linking closes that
entire class."*

The library is 12 C files, 7,538 lines, with no external dependencies (utf8proc is
vendored). It builds to a 446 KB static archive in **2.1 seconds**. There is no build cost
worth avoiding.

Disabling OpenMP removes the libgomp dependency entirely. That is a convenience rather than
the reason: the upstream failures were runtime dynamic-linking failures of a *prebuilt* `.so`,
which does not apply to a build from source. The measured case for disabling it is in
`crates/vscode-diff-sys/build.rs` — on realistic files the difference is noise, on a
pathological 20,000-line file it is ~21% wall clock at 1.31x parallelism, and diffs are
computed concurrently across files anyway, which scales better and would oversubscribe if
combined.

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
satellite of the other. Publishing `vscode-diff-sys` to crates.io is a later, optional step,
and is not a prerequisite for anything.

---

## D4 — Crate boundaries as the architectural firewall

**Decision.** Split the project into ten crates with a strictly acyclic dependency graph
declared in `Cargo.toml` **before any logic is written**.

**Rationale.** Rust modules within a crate may reference each other freely, so module
structure alone enforces nothing — exactly the situation Lua was in. Crates cannot form
cycles, and `pub(crate)` provides genuine package-private visibility, which TypeScript and
Lua both lack. Splitting is free: everything statically links into one binary, and
incremental builds get faster because only changed crates recompile.

The critical missing edge is `ui → vcs`. Because that dependency is not declared, a
renderer that shells out to git is a compile error — preventing by construction the failure
that produced a 674-line `explorer/render.lua`.

---

## D5 — Crate naming

**Decision.** No `codediff-` prefix. Crates named after the thing they contain, never after
a layer.

```
vscode-diff-sys  vscode-diff  metrics  syntax  align  explorer  vcs  runtime  ui  codediff
```

**Rationale.** `core`, `common`, `utils` and `model` are banned not on aesthetic grounds but
because they are **unfalsifiable**. "Does this belong in `align`?" has an answer. "Does this
belong in `core`?" is always yes. A crate name should state an admission criterion that can
be applied in code review.

`line-index` covers measurement and coordinate mapping — the established term for both.
`align` is named for its algorithm, not its data, because the alignment builder and the
projections are the valuable part. `runtime` states its criterion: things that exist only
while the application is running. `vcs` rather than `git` leaves room for jj and avoids the
crates.io name.

Registry uniqueness is irrelevant for path dependencies. If publishing ever happens, add
`package = "codediff-align"` to the dependency line — one line per dependent, at that time.

---

## D6 — `runtime` and `ui` state split

**Decision.** `runtime` owns domain state; `ui` owns presentation state. Events that
change *data* go to `runtime`; events that change *presentation* never leave `ui`.

**Consequence.** `j`, `k`, scrolling, folds and focus are handled entirely within `ui`
— synchronous, sub-millisecond, no channel. Only file selection, refresh and watcher events
reach `runtime`. This is the two-loop model, and it is the whole performance story.

**The follow-on problem** — preserving cursor and selection across a refresh — is solved by
keying presentation state to **stable domain identity** (`path`, `HunkId`) that the model
provides. `ui` re-resolves its own position after any refresh without `runtime` ever
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

## D11 — Syntax highlighting is in the MVP, via `syntect`

**Decision.** Included, at S11, using `syntect` (with `two-face` for bat's extended syntax
set) behind the `Syntax` trait in `crates/syntax` (see [D17](#d17--syntax-highlighting-is-its-own-crate-with-an-engine-free-interface)).

**Why it is in scope at all.** An earlier draft deferred syntax highlighting entirely, on
the grounds that it is the largest single net-new subsystem and contributes nothing to
proving the core thesis. That reasoning still holds for *ordering* but not for *scope*: the
MVP is defined as one narrow scenario experienced completely, and a diff reviewer without
syntax highlighting is not complete.

### Why `syntect` rather than tree-sitter

**The usual objection to tree-sitter no longer applies.** Grammar crates historically pinned
incompatible `tree-sitter` cores. Verified as of 2026-07, `tree-sitter-rust` (v0.24),
`tree-sitter-typescript` (v0.23), `tree-sitter-python` (v0.25) and `tree-sitter-go` all
depend only on the `tree-sitter-language ^0.1` shim, so despite the version spread they
coexist cleanly. This decision does not rest on that argument.

The reasons that do hold:

1. **We do not need tree-sitter's headline feature.** Incremental parsing exists so an
   editor can reparse on every keystroke. We highlight static snapshots — a committed blob
   and a worktree file. Nothing is being typed.
2. **~200 languages immediately, with no per-language work.** With tree-sitter each language
   needs a crate, a `highlights.scm`, and a mapping from *its own* capture names to our
   theme. Capture names vary between grammars; that mapping is the real tax.
3. **Binary size.** syntect uses one serialized syntax blob of a few megabytes. Bundled
   tree-sitter grammars are large generated C parsers — `tree-sitter-typescript` alone is
   roughly 10 MB — plus the compile time that implies.
4. **Precedent from the closest comparable tools.** `delta` — Rust, terminal, git diff
   viewer, the nearest analog that exists — uses `syntect 5.0`. `bat` uses syntect with a
   prebuilt `syntaxes.bin`. Helix and Zed use tree-sitter, but both are editors, and Helix
   ships grammars as **runtime-fetched shared libraries**, which is precisely the
   single-static-binary property we are protecting.

### What tree-sitter would buy, and the trigger to switch

Tree-sitter is more accurate on nested and injected languages (JavaScript in HTML, SQL in
strings) and recovers better from files that do not parse. Neither is decisive.

The decisive one would be **structure**: answering *"which function is this hunk in?"* and
rendering hunk context as `impl Config > fn merge`. For reviewing a sixty-file agent diff
that is genuinely valuable, and syntect cannot do it at all — there is no parse tree.

**The trigger is therefore: when we want structural features, not better colours.** The
`Syntax` trait makes the swap possible per-language rather than wholesale.

### Composition

The `SpanSet` compositor with priorities is built at S7, before any syntax spans exist, so
that compositing syntax foregrounds with diff and inner-change backgrounds is never
retrofitted.

### Languages covered by S11 acceptance

syntect supplies roughly 200 languages with no work, so the question is what to *test*. S11
fixtures cover twelve, chosen as the realistic distribution of agent-edited code plus the
configuration and markup formats that exercise composition edge cases:

```
Rust · TypeScript · JavaScript · Python · Go · Java
C · C++ · JSON · YAML · Markdown · Bash
```

---

## D12 — Stress-testing the architecture against future features

The architecture was tested against six planned agent-review features. Five require no
structural change; one forced a decision that is now made.

| test | outcome |
|---|---|
| connect to an agent backend (streaming, cancellable) | **clean** — new crate beside `vcs`, plus event variants. `RequestId` already handles cancellation |
| agent comments displayed inline against hunks | **required a decision**: `ui`'s `VisualRow` must be an enum wrapping `align::Row` with room for non-diff rows, and projections must take a context struct. Due at S7 |
| "what changed since I last looked" | **free** — falls out of `HunkId` being a content hash |
| MCP server so the agent queries the diff | **clean** — everything except `ui` and `codediff` is already headless |
| base revision = "when the agent started" | **clean** — `ContentSource::Snapshot` reserved; free if the repo is jj-backed |
| agent writes files while you review | **clean** — read-only means *codediff* never writes, not that nothing changes; the watcher already covers it |

### Risks that would force a genuine rewrite

| risk | insurance taken now |
|---|---|
| review state becomes primary and diff secondary, making `align` the wrong centre | watch for annotations gaining more fields than hunks |
| multiple simultaneous diffs (three-way, tabs, comparing two revisions) | `AppState.docs: HashMap<DocId, Document>` from the start, with one entry |
| a GUI or web frontend | already covered by the `ui` split |
| crash recovery, session replay, server-side review | `AppState` is `serde`-serializable from the start |
| `runtime` becomes a god object | `update/` submodules touch only their own sub-state; line count tracked in CI |

---

## D13 — jj support is a feature, not a nicety

**Note, not yet a decision.** jj auto-snapshots the working copy on every operation, so its
operation log answers "what did the agent change since T" **for free** — the single feature
that would otherwise require building a content-addressed snapshot store. This is the
strongest argument for the `VcsBackend` trait existing from S5 rather than being retrofitted.

---

## D14 — Split `vscode-diff-sys` and `vscode-diff`, following the `-sys` convention

**Decision.** Two crates: `vscode-diff-sys` holding the build script and the raw FFI, and
`vscode-diff` holding the safe API. This is the standard Rust `-sys` split.

*An earlier draft of this decision argued for merging them into one crate. That is
superseded — the convention wins, principally on unsafe containment.*

**What `-sys` is.** A documented Cargo convention: a `foo-sys` crate holds only raw
`extern "C"` declarations, `#[repr(C)]` types and the build script — everything unsafe and
1:1 with the C API — with a safe crate `foo` layered on top providing ownership, `Result`
and `Drop`. It declares `links = "foo"` in its manifest, and Cargo enforces that **only one
package in a dependency graph may link a given native library**, which prevents
duplicate-symbol failures when two crates independently build the same native code.

**Why we follow it.**

1. **Unsafe containment is absolute rather than conventional.** With the split, the seven
   crates that touch neither the C nor its raw pointers carry `#![forbid(unsafe_code)]` — a
   hard compiler guarantee that cannot be overridden from within the crate. In a merged
   crate the best available anywhere is `#![deny]` plus a module-level `#[allow]`, which any
   future edit can quietly widen.
2. **The unsafe surface becomes countable and auditable.** One crate, ~150 lines, that a
   reviewer can read in full. "How much unsafe does codediff contain?" has an exact answer.
3. **It is what every Rust developer expects.** `libgit2-sys`, `libz-sys`, `curl-sys`,
   `zstd-sys`, `openssl-src` all follow it. Deviating costs explanation forever.
4. **Different rebuild triggers.** `vscode-diff-sys` recompiles when the C changes;
   `vscode-diff` when the Rust changes. Splitting keeps incremental builds sharp.
5. **Publishing stays open** without restructuring, and with it the `links` guarantee.

**Layout.**

```
crates/vscode-diff-sys/
  Cargo.toml      links = "vscode_diff"
  build.rs        cc compiles vendor/libvscode-diff, OpenMP off
  src/lib.rs      #[repr(C)] structs + extern "C" declarations, 1:1 with the C API

crates/vscode-diff/
  src/lib.rs      #![deny(unsafe_code)] — public safe API
  src/convert.rs  #[allow(unsafe_code)] — ~40 lines; C → owned Rust, frees immediately
  src/types.rs    LinesDiff, LineRange, CharRange, MovedText
  src/options.rs  DiffOptions builder
```

Naming a binding after the library it binds is itself the convention (`zstd`, `curl`,
`git2`).

**Note.** `convert.rs` dereferences the raw pointers returned by `vscode-diff-sys`, so a
narrow `#[allow(unsafe_code)]` is still required there. The conversion is eager: C memory is
walked once into owned `Vec`s and freed immediately, so **no C pointer ever escapes into
application types**. That is what keeps the unsafe surface at roughly 40 lines rather than
spreading a lifetime obligation across the whole program.

---

## D15 — File watcher: `notify`, not Watchman

**Decision.** `notify` + `notify-debouncer-full` as the default and only implementation for
MVP, behind a `WatcherBackend` trait. Watchman is an optional backend to be built only on
demand, auto-detected from `PATH`.

**Why Watchman was rejected as primary.**

| crate | version | last updated | recent 90d downloads |
|---|---|---|---|
| `watchman_client` | 0.9.0 | 2024-06-18 | 425,754 |
| `notify` | 8.2.0 | 2026-05-02 | 34,564,519 |

1. **It is a daemon the user must install.** That contradicts the core value proposition of
   a single static binary that works over SSH with nothing else present.
2. **The Rust client is over two years stale**, against an actively maintained `notify` with
   81× the usage.
3. **It is tokio-based**, conflicting with [D8](#d8--no-async-runtime).
4. **The scale problem it solves has largely evaporated on Linux** — the modern
   `max_user_watches` default is 524,288, not the 8,192 that produced the exhaustion
   folklore.
5. It is heavy machinery — a per-user daemon with its own state directory, lifecycle and
   version-skew modes — for watching one repository.

Watchman genuinely wins on 1M+ file monorepos and with its `since <clock>` queries, but
anyone on such a repository already has it installed and running, which is exactly what
auto-detection exploits.

**Note.** codediff.nvim independently reached the same conclusion for a different reason.
Upstream issue #482 evaluates the same landscape and rejects the Rust `notify` crate solely
because it *"adds Rust toolchain to the release pipeline (currently just C++)"* — a
packaging constraint of being a Lua/C plugin, not a technical judgement. Their analysis
otherwise endorses this class of solution.

---

## D16 — Watcher design, informed by upstream production failures

Upstream issue #482 and PR #480 document three successive watcher designs in production,
with measurements. Every lesson below is earned, not theorised, and is adopted directly.

### The three states upstream went through

| design | measured outcome |
|---|---|
| watch `.git/` only | **self-triggering loop.** Their own `git status` momentarily writes `.git/index.lock`, which wakes the watcher, which runs `git status`. ~20 refreshes / 10 s forever, **~120 git subprocesses/min, ~290 ms nvim CPU / 10 s while completely idle** |
| add a `*.lock` event filter (#480) | loop fixed — 0 idle refreshes, ~6 ms CPU / 10 s. But it **silently removed detection of working-tree changes**, because the loop had been *accidentally functioning as a poller*, and the filter also suppressed the `index.lock → index` rename that carries the real signal |
| explicit 500 ms poll (current) | correct behaviour restored, but the subprocess and CPU cost returns |

### Decisions adopted from this

**1. Watch both the worktree and `.git/`.** Watching only `.git/` cannot see
`touch new_file.txt`. This is exactly the #480 regression. VSCode does both — a `.git/`
watcher plus a workspace-wide recursive watcher — and so will we.

**2. Filter lock files by *destination*, not by path substring.** A `*.lock` path means only
that a git operation is in flight; the state change arrives afterwards. But git writes
`index.lock` and then **renames it onto `index`** — that rename *is* the signal. A naive
"path contains `.lock` → ignore" rule drops it, which is precisely what broke #480. With
`notify-debouncer-full` a rename arrives as `RenameMode::Both` with `from = index.lock`,
`to = index`: **filter on the destination path.**

**3. Prevent self-triggering structurally, not heuristically.** We know when we spawn a git
subprocess. Suppress watcher-driven refresh for its duration rather than trying to recognise
its side effects after the fact. This closes the feedback loop by construction.

**4. Watch directories, not files.** inotify watches are per-directory and cover every file
directly inside. Measured: codediff.nvim is 58 non-ignored directories against 332 tracked
files; a 165,000-file tree is ~28,000 directories, against a 524,288 watch limit. Recursive
directory watching is therefore cheap *and* gives instant new-file detection.

**5. Watch `.git/` non-recursively.** Recursive would place watches across `.git/objects/`'s
256 fan-out directories and produce an event storm on every git operation. Note the notify
caveat that a directory deletion is only observed by watching its *parent*, so to see
`.git/rebase-merge/` disappear we watch `.git/` itself.

**6. No routine polling.** Upstream's acceptance criterion is **zero git subprocesses while
idle**, which an interval poll cannot satisfy. This supersedes the safety-net poll proposed
in an earlier draft of this plan. Instead:
   - handle `EventKind::Other` with `Flag::Rescan` (inotify queue overflow) as a full status
     re-read
   - fall back to `PollWatcher` only when the native watcher fails to initialise or reports
     too many watches — matching watchexec's native-or-poll model
   - offer an explicit opt-out for battery-sensitive users

**7. Debounce ~50 ms** so `git checkout`-scale bursts collapse into one refresh, while
staying inside the 100 ms latency target.

**8. Exclusions.** Upstream chose glob-based `watcher_exclude`, following VSCode's
`files.watcherExclude`. We can do better cheaply: respect `.gitignore` via the `ignore`
crate *and* accept globs.

### Acceptance criteria — adopted verbatim from #482

These are upstream's, and they are measurable:

- working-tree edit (`touch`, `echo x >> file`) surfaces within **100 ms**
- external `git commit` surfaces within **100 ms**
- idle CPU **≤ 5 ms per 10 s**
- **zero git subprocesses fired while idle**
- graceful fallback to polling on watcher failure or watch-limit exhaustion
- opt-out configuration
- reliable cleanup on exit; no orphaned threads

### What this costs us versus upstream

Upstream's plan (issue #482, labelled `Size/XL`, ~1 week focused) is: vendor the `efsw` C++
library, write a ~200 LOC C shim, write a ~150 LOC Lua FFI binding, extend the release
matrix to build and ship a **second** native binary for six platforms, extend
`installer.lua` to fetch it, statically link `libstdc++`/`libgcc`, and build on
manylinux2014 for glibc compatibility — then inherit efsw's ~50 open issues.

Ours is two lines of `Cargo.toml`.

This is one of the clearest single wins of the rewrite, and it exists only because the whole
project is already Rust — the exact reason upstream could not take this path.

---

## D17 — Syntax highlighting is its own crate, with an engine-free interface

**Decision.** Highlighting lives in `crates/syntax/`, not as a module inside `ui`, and
its public interface never names a syntax engine.

**Why not a module in `ui`.**

1. It is not a rendering concern. Highlighting is text *analysis* — text plus language
   produces spans; rendering *consumes* spans. Conflating them is the same category error
   that produced a 674-line `explorer/render.lua`.
2. `ui` would carry a heavy dependency (syntect and two-face, or tree-sitter and N
   grammars) that nothing else in the workspace needs.
3. Other consumers are coming — agent export and `--dump-frames` want syntactic information
   without rendering anything.

It also meets the extraction rule stated in [D4](#d4--crate-boundaries-as-the-architectural-firewall):
extract when a module acquires a distinct dependency set. This one does, decisively.

### The interface must sit above the engines' computation models

The obvious trait leaks the engine and is unswappable:

```rust
// BAD
fn highlight_line(&self, line: &str, state: &mut syntect::ParseState) -> Vec<(Style, &str)>;
```

syntect and tree-sitter compute differently in kind:

| | syntect | tree-sitter |
|---|---|---|
| model | stateful, line by line, carrying a parse stack | parse the whole file to a tree, then query |
| access | must process lines in order | random access to any node |

A line-oriented interface bakes in syntect's model, and tree-sitter cannot implement it. So
the interface is whole-file-in, spans-out:

```rust
pub trait Syntax: Send + Sync {
    fn spans(&self, text: &FileText, lang: Language) -> SpanSet;
}
```

### The crux: a normalized `Class`, not engine scopes

This matters more than the crate boundary. syntect emits Sublime scopes
(`keyword.control.rust`); tree-sitter emits capture names (`@keyword.control`). Passing
either through raw couples the **theme** to the engine, so swapping engines would break
every colour in the application.

`syntax` therefore owns a normalized vocabulary of roughly sixteen classes:

```rust
pub enum Class {
    Keyword, Type, Function, Variable, Constant,
    String, Number, Comment, Operator, Punctuation,
    Attribute, Namespace, Property, Tag, Escape, Error,
}

pub struct Span { pub line: LineIdx, pub range: Range<ByteOff>, pub class: Class }
```

Both engines map *into* it. `ui`'s theme maps `Class → Style`. The split is clean:
**`syntax` says what something is, `ui` says how it looks.**

### The three conditions that make the swap free

Replacing syntect with tree-sitter touches only files inside `crates/syntax/` — if and only
if:

1. the trait is whole-file-in / spans-out, hiding stateful versus tree-based computation
2. `Class` is normalized; no engine scope string ever escapes the crate
3. no engine type appears in any public signature

Condition 3 is mechanically checkable: `cargo xtask lint-arch` fails if `syntect::` or
`tree_sitter::` appears outside `crates/syntax/src/engine/`.

```
crates/syntax/
  lib.rs          Syntax trait, SpanSet, Class — public, engine-free
  language.rs     detection by extension, shebang, content
  engine/
    mod.rs
    syntect.rs    the ONLY file permitted to import syntect
  map.rs          engine scopes → Class
```

### Caching lives in `runtime`, not in `syntax`

syntect on a 5,000-line file takes 50–200 ms, far too slow for a frame. So `syntax` stays
pure and stateless — its syntax set is `include_bytes!`, not IO, so it joins the pure tier —
while `runtime` owns a cache keyed by `(path, content_hash)` and runs highlighting as a
Loop B effect.

The first frame therefore paints unhighlighted and repaints when spans arrive, which is how
editors behave. All caching stays in one place, and highlighting stays off the render path.

---

## D18 — `align` matches VSCode's model: nothing per row, nothing copied

**Decided at S4.** The pairing is `Alignment`, which borrows the `LinesDiff` and both files and
computes every answer on demand. It stores no rows, no text and no derived index.

The design it replaced was `AlignedDoc { rows: Vec<Row> }` with `Row { left, right, kind }`
and `RowKind::MovedFrom/MovedTo`. That was written before reading either reference
implementation, and reading them collapsed it:

- **VSCode's `DiffState` is four fields** — the engine's mappings, its moves, `identical`,
  `quitEarly`. Its alignment entries are `{ originalRange, modifiedRange }` plus two pixel
  heights that exist only for line wrapping and plugin-inserted view zones. Strip those and
  what remains is already in our `LinesDiff`.
- **VSCode emits alignment entries only at changes.** Unchanged stretches are implicit,
  carried by a running offset.
- **`DiffMapping.movedTo` / `movedFrom` are commented out in the VSCode source.** They tried
  attaching moves to changes and abandoned it.

Four defects in the replaced design, in order of severity:

| defect | consequence |
|---|---|
| `MovedFrom`/`MovedTo` as row kinds | **wrong output.** In `comprehensive_move` a move covers original 32..89 while a change covers 37..139. Move ranges need not agree with change ranges, so a move cannot be a property of one |
| `left` / `right` | bakes layout into the model; inline view draws both sides in one column and contradicts it |
| one entry per row | grows with file size, not edit count. Most entries were `Unchanged` — pure derivable padding |
| stored beside `LinesDiff` | two structures that can disagree, rebuilt on every save by a watcher-driven tool |

**What we add that VSCode does not need.** Its editor answers "what is on screen row *n*";
we have no editor, so `rows()` expands ranges into lines at draw time — a walk, not a stored
structure. And the engine reports UTF-16 columns, which JavaScript takes for granted and
Rust does not, so inner changes go through `line_index::utf16_range_to_bytes`.

**Ownership.** `Alignment` borrows the `LinesDiff` and both files, so it is a view and never a
stored field. Every link in the chain — file contents, then line vectors, then the
`LinesDiff`, then the alignment — borrows from the one before it, so nothing can return the
whole chain: each link would have to outlive the value returned with it. The pipeline
therefore returns everything **owned** and lends the borrowed part to a closure
(`Runner::run`), rather than making the caller sequence three locals correctly. See D26.

**Cost accepted.** Locating a row is a walk over the changes rather than an array index.
Rendering asks for consecutive rows, so this is one pass per frame — seven iterations on
`comprehensive_move`. A prefix sum would make it `O(log n)`, is derivable from the changes,
and can be added without changing the interface if a file with thousands of changes ever
makes a scrollbar drag feel slow.

---

## D19 — the container owns the row index, so scroll sync cannot exist

**Decided at S4, before building `ui`.** A `View` owns one vertical position; a `Pane`
owns only its width, gutter and row source.

```rust
enum Layout { SideBySide { left: Pane, right: Pane, split: u16 }, Inline { pane: Pane } }
struct View { layout: Layout, row: u32, subrow: u16, h_scroll: CellCol, cursor: Cursor }
```

Side-by-side draws row *n* in both panes — left takes `row.original`, right `row.modified`.
They cannot drift.

VSCode instead gives each side a `CodeEditorWidget` that owns its own `scrollTop`, pads the
shorter side with view zones until the heights match, then holds the two scroll values in a
bidirectional constraint with write guards to stop feedback. It needs two editors because
each side is *editable* and wants its own cursor, selection, find and folding. Read-only,
none of that applies. The plugin spent 415 lines (`scrollsync.lua`) on the same problem and
got it wrong twice.

**Alignment with heights belongs in `ui`, not `align`.** Once wrapping exists a line is
no longer one row, so pairing depends on pane width — which is why VSCode computes
`ILineRangeAlignment` in its *view* while `DiffState` stays width-independent. `align` keeps
the width-independent pairing; `ui` computes row counts at the current width and pads
after a range on the shorter side. `LineRangeAlignment { original, modified, original_rows,
modified_rows }` is the right name for that, in `ui`, at S10a.

**Costs accepted.**

- **Draggable split.** Unequal panes mean identical unchanged text wraps to different heights
  on each side, so wrapping needs VSCode's `handleAlignmentsOutsideOfDiffs` checkpoints. Only
  under wrap, which is opt-in.
- **No auto-switch to inline.** VSCode flips to inline below 900px by default; we do not.
  Horizontal scroll is the answer to a narrow terminal, matching the plugin, which sets
  `wrap = false` in all six of its windows. `Layout::Inline` exists but is unused at MVP.

**Also settled:** folding does not feed back into alignment either — VSCode calls
`setHiddenAreas` on the editors and leaves `computeRangeAlignment` alone. Folding and inline
are both projections over the same pairing. Inline must group by hunk, emitting a hunk's
deletions then its insertions; a row-by-row walk of side-by-side pairs would interleave them
wrongly.

---

## D20 — type names mirror the C header

The C engine is a faithful port and already carries VSCode's vocabulary, so one rule settles
naming in `vscode-diff`: **our Rust types mirror `vendor/libvscode-diff/include/types.h`,
which mirrors VSCode.**

| was | now | C header |
|---|---|---|
| `LinesDiff` | `LinesDiff` | `LinesDiff` |
| `Change` | `DetailedLineRangeMapping` | `DetailedLineRangeMapping` |
| `Move` | `MovedText` | `MovedText` |
| `change.inner` | `inner_changes` | `inner_changes` |
| `LineRange { start, end }` | `{ start_line, end_line }` | `{ start_line, end_line }` |

`CharRange` and `RangeMapping { original, modified }` already matched. The rule also settles
what *not* to add: VSCode's `IDocumentDiff` has an `identical` flag, the C header does not,
and it is derivable from `changes.is_empty()`.

In `align`, `Region` became `UnchangedRegion`, matching VSCode. `Row`, `Slot`, `Hunk` and
`Side` have no counterpart in either and keep their own names. `Alignment` is deliberately
*not* renamed to `LineRangeAlignment`: there that name means one entry of an array, here it
would mean the whole model.

The cost is verbosity — `DetailedLineRangeMapping` is a mouthful. The rule is worth more than
the keystrokes, and it is checkable rather than a matter of taste.

---

## D21 — `vcs` runs `git` rather than linking a git library

**Decided at S5.** `gix` and `git2` are real options — `gix` has 40M downloads and ships
regularly — so this is a choice, not an absence of one.

**Speed is not the reason.** Measured: `git --no-optional-locks status --porcelain=v2 -z`
on a 340-file repository is **~4.5 ms**, twenty runs in 91 ms. That is far below the
refresh rate a watcher will ask for.

The reason is that git's own binary already honours the user's config, `.gitignore` rules,
linked worktrees, sparse checkout and clean filters. Those rules decide **which files
appear at all**, so a reimplementation that differs anywhere shows the wrong list — the one
kind of wrong a review tool cannot afford. `Diff` is a trait, so this is reversible, and a
future `jj` backend needs one anyway.

**Two layers, each in its own language.** The `Diff` trait is in the reviewer's terms —
`files()`, `before(file)`, `after(file)` — and names no git concept, because a system need
not have one: jj has no index and no `HEAD`. Underneath, `git/` keeps every git word, with
modules named for the commands they run, and `git::to_file_diff` is the single point the
two vocabularies meet.

**One folder per capability, each holding its trait and the types in its signatures.**
`changes/` today; `staging/` and `history/` when something needs them. `repo` and `error`
sit above them all. A crate named for a whole domain otherwise becomes a place to put
anything, which is how the Lua explorer got to 674 lines in one file.

It was called `Diff` rather than `Change` because the engine already reports **line**-level
changes. That was the wrong trade: the name was borrowed from the one git command the crate
never runs. It is `Changes` now, and `ChangedFile` carries no such ambiguity —
[D29](#d29--vcschanges-because-nothing-there-diffs-anything).

Forcing one vocabulary on both would go wrong in either direction — inventing fake-neutral
names for git things, or making a jj backend pretend it has a staging area.

**Capabilities that only some systems have get their own traits.** `Staging` and `History`
would sit beside `Diff`, so a backend lacking one fails to compile rather than answering
"unsupported" at runtime. Only `Diff` exists today. Note that staging is *not* excluded for
being a write: it never changes file content, so it stays within what this tool does.
Restoring, discarding and resolving a merge do change content, and those are the line.

The equivalent layer in the plugin has 26 functions. Thirteen — comparing arbitrary
revisions, history browsing, rename following — are real reads we will want later but not
for worktree-vs-HEAD.

**Three details that break naive implementations,** all found by running git rather than
reading about it:

| | |
|---|---|
| a **rename record spans two NUL-terminated fields** | splitting the stream on NUL and treating each piece as a record turns one rename into a record plus a garbage entry |
| **`--no-optional-locks` goes before the subcommand** | as a `status` flag it is rejected. It stops git taking `.git/index.lock` for the optional index refresh, which would both fail a concurrent `git add` and wake the watcher that asked for the status |
| **field offsets differ per record type** | `1` has two hashes, `2` adds a similarity score, `u` has three stages and so three modes and three hashes. Counting wrong puts a hash in the path, which the fixture caught |

**Blobs come from one long-lived `cat-file --batch`.** A sixty-file diff is a hundred and
twenty reads; at a process spawn each that is most of a second in `fork`. The child is
stateful, so it gets its own thread rather than a slot in a pool sized for computation.

**The `fixtures` crate has no workspace dependencies** so `vcs` tests and, later, end-to-end
tests can dev-depend on it without a cycle. Its manifest is written by hand from
`git-status(1)`; one generated from our own output would only prove the parser agrees with
itself.

---

## D22 — Catppuccin by arithmetic, with a theme that cannot fail beside it

The plugin has no palette. Its diff colours are read out of whatever colourscheme Neovim is
running — `DiffAdd`, `DiffDelete`, `DiffChange`. Standing alone, we have to choose.

**A theme is a table of `Style`s, one per role.** Not colours: a `Style` also carries bold
and reversed, which is what lets a theme work on a terminal that has no colour to give.
They compose by `Style::patch`, which overrides only the fields that are set, so a role
supplies a background and inherits the foreground, and priority is the order the patches
are written in rather than a table of numbers nobody can read.

**Catppuccin is reproduced by its arithmetic.** Its diff backgrounds *are* a function of
its palette:

```text
out = round(alpha × accent + (1 − alpha) × base)

DiffAdd  18% green    DiffChange   7% blue
DiffDelete 18% red    DiffText    30% blue    CursorLine 64% surface0
```

So `theme/catppuccin.rs` holds four 26-colour palettes and one `const fn` derivation, and
a flavour is 26 numbers rather than 26 plus fourteen more that must be kept in step. A test
asserts the derivation still reproduces Catppuccin's own published values, so a theme
claiming to be Catppuccin stays one.

Two consequences worth stating:

- **Inner changes use `DiffText`'s ratio, not the plugin's multiply.** The plugin brightens
  the line colour by 1.4 — a direct RGB multiply, no alpha — which on a light background
  darkens rather than brightens and on a saturated one clips. A second blend at a higher
  opacity behaves the same way on all four flavours, and a test checks that on each.
- **There is no "modified" colour.** A modification is red on the original side and green
  on the modified one, which is what a side-by-side view means: each side says what
  happened to *it*.

**`basic` exists because Catppuccin's subtlety is also its failure mode.** Eighteen percent
of an accent is a few points of lightness. A terminal without 24-bit colour quantises that
straight back into the background, and the result is a diff viewer with no visible diff in
it — the worst possible failure, because it looks like it worked. So there is a second
family that names nothing exactly: `Color::Reset` for the background, so it inherits
whatever scheme the reader already runs, and the 256-colour cube for the diff backgrounds.
A test asserts it never emits a 24-bit colour at all.

**Detection is from the environment, and one-way.** `COLORTERM` is what every terminal
supporting 24-bit colour sets, and `COLORFGBG` is the existing convention for light
backgrounds; unset means "not sure", and being unsure is a reason to pick the theme that
cannot fail. There *is* a real way to ask — an OSC 11 query — but it needs a round trip the
terminal may never answer, and a reviewer waiting on a timeout before the first frame is
worse than a wrong guess they can override with `--theme`. `codediff doctor` prints what
was detected, because "my colours are wrong" is otherwise unanswerable by looking.

---

## D23 — a file with only one side is shown in one pane, not diffed against nothing

The engine models an empty file as **one empty line**, so `compute(&[], x)` is normalised to
`compute(&[""], x)`. `align` normalises identically, or an `Alignment` would disagree with
its own diff — found by proptest, which shrank to `original = []`.

That is right for a file that exists and is empty. It is wrong for a file that does not
exist. An added file rendered that way gets a phantom blank line paired against its first
real line, and reports as *modified* rather than added. It bit us twice, at S4 and S6.

**VSCode hit exactly this bug and fixed it the same way.** From the maintainer who closed
[microsoft/vscode#239914](https://github.com/microsoft/vscode/issues/239914) as
*as-designed*:

> the file system provider that handled the `git` scheme used to return an "empty string"
> for a file that did not exist. This implementation made it impossible to differentiate
> between a file that did not exist and an empty file… Untracked files previously used to
> open in the diff editor with the left hand side being empty. As [it] did not provide much
> value, untracked files are now opened in the normal editor instead of the diff editor.
> This matches the behaviour for deleted files.

The rule in their source is one line, `git/src/repository.ts:535`:

```ts
if (!leftUri) → vscode.open   // one pane
else          → vscode.diff   // two panes
```

and `getLeftResource` returns a left side only for `MODIFIED`, `INDEX_MODIFIED`,
`INDEX_RENAMED`, `INTENT_TO_RENAME`, `TYPE_CHANGED` and the two conflict cases. Added,
untracked and deleted all fall through to `return {}`.

**So we do the same.** A one-sided file is not compared against anything:

| | |
|---|---|
| `Opened::compare` | returns an **empty diff** when a side is absent — no engine call |
| `Opened::lines_to_show` | the present side stands in for both, so every row is unchanged |
| `Layout::Single(Side)` | one pane at full width; the other side is never read |

Nothing is highlighted, because nothing changed relative to anything — there is no other
side to be relative to. Marking every line of a new file green says nothing that the word
"added" does not.

Three consequences worth stating:

- **The decision is `absent`, never `empty`.** A tracked file emptied to zero bytes still
  has a side to compare against, so it gets a real two-pane diff showing every line
  deleted. Verified on a terminal, alongside the one-pane cases.
- **A deleted file shows its HEAD content** — what was removed — which is what VSCode's
  `getRightResource` does for `DELETED`.
- **The status line says `(added)` or `(deleted)`.** VSCode does not need to: it leaves the
  diff editor for an ordinary tab, and the tab is the cue. We have nowhere else to go, so
  the single pane *is* that, and it needs a label or it reads as an unchanged file.

An earlier attempt made the one-sided file into a *diff* — `LinesDiff::one_sided` plus
`Alignment::try_verbatim` — and rendered it as two panes with a column of fillers. Both
were reverted. The data was defensible; drawing it as two panes was not.

---

## D24 — a key resolves to one of three kinds of command, and resolving is not dispatching

`ui` had one flat `Intent` enum whose variants were answered by four different owners.
The symptoms were measurable: `Intent::Quit` was answered **twice** — the view returned
`false` for it while the loop intercepted it first — `Intent::Redraw` was a no-op that
worked only because the loop redrew unconditionally, and `View::focus` was a field nothing
outside its own file ever read.

**The split is by who answers, and how long they take** — not by whether there is a side
effect, because that question does not tell the loop what to do:

| | answered by | can fail | latency |
|---|---|---|---|
| `View` | `ui`, in this frame | no | µs |
| `Program` | whoever owns the terminal | no | µs |
| `Task` | the composition root, off-thread | **yes** | ms |

**A `Task` is a request, not a call.** `ui` names what it wants; something above
performs it and returns the answer as an event. That is the only way staging can exist
without `ui → vcs`, which `lint-arch` forbids. It is deliberately uninhabited: after
startup this binary performs no IO, so nothing a key could ask for exists yet. The variant
and its dispatch arm are written now so the explorer *adds* one rather than reshaping the
loop.

**Resolving and dispatching are separate.** `input/` turns keys into a `Command` and
returns; `app.rs` sends it to whichever of the three can answer. A single "engine" doing
both would need references to the viewport, the terminal and the task runner at once —
exactly the coupling the split exists to prevent. The payoff is that the resolver is a
**pure function of its own state and one key**: no clock, no IO, no view, and a test is a
string of keys.

**The table is data, not closures.** `crokey`'s `key!()` is const-capable, which is the
reason to depend on it: the bindings are a `const` list that can be printed into a help
screen, walked by a test, and checked for prefixes. A closure could do none of those, and
would capture references to everything it might touch.

**crokey covers what a key *is*; sequences are ours.** Its "combination" means keys pressed
together (`Ctrl-Alt-g`); `gg` is keys pressed one after another. Different axis. It also
supplies `KeyEvent` conversion, help-screen formatting and — later — config parsing, and it
shares our crossterm 0.29 so nothing duplicates.

**No binding may be a proper prefix of another.** Commands live only at the leaves of the
trie. This is what vim's own built-ins already do — `g`, `d`, `z`, `[`, `]` are unbound
alone — and it is why the resolver needs no clock. Ambiguity has no good resolution: firing
immediately makes the longer binding unreachable, and waiting makes the shorter one feel
broken for half a second every time. Vim needs `timeoutlen` only because user mappings
*may* create ambiguity. Enforced by a test rather than assumed by the resolver, so relaxing
it later means adding an injected clock and deleting one test.

Two rules fall out of that, both vim's:

- **Escape cancels what is in flight, and only then.** With nothing pending it reaches the
  table, where it quits. Without the interception, pressing `g` and changing your mind
  exits the program — and `5` then Escape quits *with a count of five attached*.
- **`0` is a digit once a count has started and a motion otherwise** — the only point at
  which counts and bindings interact.

`View` was renamed `Viewport`, which is what it holds, freeing the name for the command
kind. `Tab` and `focus` were deleted rather than left dead; they return when focus is real.

---

## D25 — what a diff *is* lives apart from the engine that computes one

`align` never calls the diff engine. It is handed a result and works out where the fillers
go — pure, no IO, proptest-tested. But it has to *name* that result, and while the six
structs lived in `vscode-diff`, naming them meant this:

```text
align → vscode-diff → vscode-diff-sys → cc → libvscode-diff.a
```

So a clean `cargo build -p align` compiled C. Measured: **4.2s, versus 0.7s now.**

The structs moved to **`diff-types`** — `LinesDiff`, `LineRange`,
`DetailedLineRangeMapping`, `RangeMapping`, `MovedText`, `CharRange` — with no
dependencies, no build script and no `unsafe`. `vscode-diff` depends on it, keeps
`compute`, and re-exports the types so an existing caller needs only one dependency.

**The counter-argument, and why it was wrong.** [D20](#d20) says our type names mirror the
C header, so the types are engine-shaped rather than neutral — which sounds like a reason
to leave them in the engine's crate. It is not. Mirroring the header is about *naming*, so
that a question about our behaviour can be answered by reading VSCode's source. Nothing in
those structs mentions C, and a second engine — a pure-Rust fallback, or a WASM build where
`cc` cannot run — would produce these same values.

**Tests may use the engine; the library may not.** `align`'s tests feed real engine output
through the aligner: twelve vendored fixture pairs, and proptest cases built from actual
diffs. Those are the tests worth having, so `vscode-diff` is a **dev-dependency**. Dev
dependencies do not propagate, so consumers still get no C.

That distinction needed a new kind of lint rule. `FORBIDDEN_SHIPPED_EDGES` checks
`[dependencies]` and `[build-dependencies]` only, while the existing `FORBIDDEN_EDGES`
checks all three tables. Sabotage-verified in both directions: moving `vscode-diff` into
`align`'s `[dependencies]` fails, and leaving it in `[dev-dependencies]` passes.

## D26 — one pipeline, five stages, and the interface `ui` receives

Assembly used to be `open.rs` plus `review.rs`: three free functions, two methods, and four
locals the caller had to sequence in the right order with nothing checking it. Every stage
existed, but there was no pipeline — it was a toolkit.

**The five stages, named and in order:**

| | file | |
|---|---|---|
| 1 | `resolver` | which file, in which repository |
| 2 | `contents` | read both sides |
| 3 | `diff` | call the C engine |
| 4 | `diff` | pair the lines up |
| 5 | `runner` | hand over a `ui::Diff` |

Five stages, five files, and `mod.rs` holds only the signpost. The first
attempt left stage five inside `mod.rs`, where the folder listing did not show
it — the same defect as burying the key resolver in `input/mod.rs`, made twice
in one session.

The files are **nouns**, per the hard rule in
[02-architecture.md](02-architecture.md) — a type owns its logic, and
verb-splitting is what produced the plugin's `actions`/`render`/`refresh`
triplets. The first attempt named two of the four after verbs (`resolve.rs`,
`compare.rs`) and the third after nothing in particular (`sources.rs` — sources
of what?). `resolver` also matches `ui/src/input/resolver.rs`: same word,
same job, one convention.

Stage 5 did not exist before; the work was scattered through `review.rs`, which is why
`ui` needed two constructors and the caller had to know which to call.

**It lives in the binary.** `codediff` is the only crate allowed to name `vcs`,
`vscode-diff`, `align` and `ui` together — `lint-arch` forbids those edges everywhere
else. A renderer that could assemble its own input is a renderer that can shell out to git,
which is what produced a 674-line `explorer/render.lua` in the plugin.

**`ui` defines what it consumes.** `ui::Diff { label, alignment, sides }`, with
`Sides = Both | Only(Side)`. The consumer defining its own input is the direction that
keeps the graph acyclic: the composition root already depends on `ui`, whereas
`ui` naming a type from the pipeline would be a cycle. `Session::new` and
`Session::single` collapse into one constructor, since how many panes to draw now follows
from `sides` rather than from which function was called.

`Sides` is a **fact**, not a layout: `ui` turns `Only(s)` into `Layout::Single(s)`
itself. It cannot work this out from the alignment, because a one-sided file is
deliberately paired with itself and so looks exactly like an unchanged comparison (D23).

**The last stage took a closure, and no longer does.** Every link borrowed the one before
it — contents, line vectors, `LinesDiff`, alignment — so nothing could return the whole
chain, and `Runner::run` lent its result out instead. An intermediate `Prepared → Ready →
Diff` was tried first and was worse than the four locals it replaced.

That was a symptom, and it was misread as a constraint for a long time: **a stage that
cannot return its own output is not a stage.** [D27](#d27--a-neovim-shaped-view-view--tab--pane--buffer)
removed the cause — `Alignment` now owns the two files it describes — so every stage
returns, and the five stages are five again.

```rust
let runner = pipeline::Runner::new(&request)?;
let mut session = ui::Session::new(runner.run()?, theme);
```

**`Alignment::diff()` is gone.** It read like a verb — "alignment computes a diff" — when it
was a getter handing out the borrowed engine result. VSCode has no equivalent because
`DiffState.fromDiffResult` *unpacks* the four values and drops the result, so there is
nothing left to reach into. We borrow rather than copy, but the surface now matches:
`changes()`, `moves()`, `hit_timeout()`, replacing seven `alignment.diff().field`
reach-throughs.

**`codediff <path>`, not `codediff review <path>`.** The plugin has no `review` command:
`:CodeDiff` *is* the diff, arguments say what to compare, and subcommands are other modes
(`history`, `merge`, `install`). `review` was scaffolding invented because the explorer
does not exist yet, and it had leaked into the CLI surface. One consequence, caught by a
test: a bare word is now a path, so `codediff not-a-command` exits **1** (no such file)
rather than **2** (bad command line).

## D27 — a Neovim-shaped view: View → Tab → Pane → Buffer

**The problem.** One `Session` held one `Diff` and one `Viewport`. An explorer needs a
second thing on screen with its own position, its own keys, and its own contents, and there
was nowhere to put any of it. Adding a field per feature is what produced the plugin's
20-field session struct and Zellij's acknowledged 80-field `Tab`.

**The shape.** Four levels, each containing the next:

```text
View     tabs, and every buffer any of them can show
└ Tab    a layout of panes, and which has focus
  └ Pane one buffer, and one Viewport onto it
    └ Viewport   top, cursor, left, split
```

Buffers live in `View`, referenced by `BufferId`, never by reference: a pane holding `&mut
Buffer` makes the whole structure self-referential. Helix does exactly this with
`DocumentId`/`ViewId`. Zellij's `Box<dyn Pane>` is the counter-example and forced
`Rc<RefCell<_>>` throughout, because two panes cannot be borrowed mutably through trait
objects.

**The module tree is the diagram.** Four levels, four files, in containment order:
`view/{mod,tab,pane,viewport}.rs` with `view/buffer/` inside. `buffer/` began as a sibling
of `view/` and that was an accident of the order the files were written — the tell was that
`view` named `crate::buffer` while all three files of `buffer` named `crate::view`. Two
siblings each reaching for the other are one thing that got split.

Neovim's buffers are global and Helix keeps `documents` beside its `tree`, so a sibling
arrangement has precedent — but in both cases a third thing above (the editor) owns both.
`View` owns `buffers` directly, so nesting is what the code already says. The alternative
was inventing an owner to justify a directory.

An id lives with the collection it indexes: `BufferId` beside `View::buffers`, `PaneId`
beside `Tab::panes`.

Position lives on the **pane**, not the buffer, so two panes over one buffer scroll
independently — the same reason Neovim splits window-local options (`wrap`, `number`,
`cursorline`) from buffer-local ones (`filetype`, `tabstop`).

**A buffer is a sequence of rows you can scroll through.** That is the whole definition, and
it settles a question that had been open since S7: side-by-side and inline are *different
buffers* over the same diff, not one buffer with a flag. They emit different row sequences,
so with a flag "row 40" would mean different things depending on a field stored elsewhere.
With two kinds, row space is fully determined by the buffer.

Which is why a buffer is a **projection** and not the data. `ui::Diff` — what the
pipeline delivers — is one file's two versions and the pairing between them, and carries no
row count: an `align::Row` is a *pair*, so a row count is already an answer to "how would
this look side by side". `SideBySide` holds that answer next to the decision that produced
it, and nothing else can hold a number that depends on a layout it did not choose.

The kinds are an `enum`, not a trait. Exhaustive `match` means adding one breaks the build
until it is handled everywhere — the same property that stops the keymap growing dead
commands.

### The borrow that shaped four layers, and how it was removed

`Alignment` used to borrow: `&LinesDiff`, `&[&str]`, `&[&str]`. That single fact reached
further than any other decision in the project.

A borrowed alignment cannot outlive the function that builds it, so **stage 5 of the
pipeline could not return its own result.** It took a closure instead — "I cannot hand you
this, but I will call you while I still hold it":

```rust
pub fn run<R>(&self, f: impl FnOnce(Diff<'_>) -> R) -> R
```

And every type that held one inherited the lifetime:

```text
Alignment<'a> → Diff<'a> → Session<'a> → View<'a> → Tab<'a> → Pane<'a>
```

The first attempt at this decision worked *around* that: buffers held plain data and the
renderer rebuilt an alignment each frame. It was measured and cheap — 2–58 µs, O(changes)
rather than O(file size) — and it was still wrong, for reasons no measurement could show:

- **The pipeline stopped being a pipeline.** Stage 4 was deleted and its work scattered into
  `DiffData::new` and `render::diff`, in another crate. Four stages produced something the
  renderer then had to finish.
- **The work was done twice and thrown away once.** `DiffData::new` computed the hunks, read
  two numbers off them, and dropped them; the renderer recomputed them every frame.
- **Its own justification was circular.** The measurement was of waste, reported as a budget.

The fix is at the source. `Alignment` **owns** its two files and the diff:

```rust
pub struct Alignment {          // no lifetime
    diff: LinesDiff,
    original: Vec<String>,
    modified: Vec<String>,
    tab_width: u8,
    hunks: Vec<Hunk>,
}
```

Every consequence unwinds. `ui::Diff` is the original struct minus `<'a>`. Stage 4
returns to `pipeline/diff.rs`; stage 5 returns rather than lends; the closure is gone; no
type in `ui` has a lifetime; and drawing a frame does no derivation at all.

The price is one copy of each file, once, at open — a few hundred microseconds for a 20k-line
file. `align`'s original reason for borrowing survives: the lines are copied *in* and the
caller's are dropped, so there is still exactly one copy to fall out of step with nothing.

The four public functions that take lines are now generic over `S: AsRef<str>`, so tests
still write `&["a", "b"]` while `Alignment` holds `Vec<String>`.

**What a walk is still needed for** — the row count and where the changed blocks sit — is
computed once in `Diff::new` and remembered, beside the `hunks` the alignment computed at
construction.

### The executor rule

> An action is executed by the **lowest level that contains everything it affects.**

A motion affects one viewport → the focused pane's buffer does it. The split between a
diff's two columns is inside one pane → the buffer again. Resizing a pane border affects
**two** panes → only the tab contains both, so the tab must. That is why resize felt awkward
to place: it is the first command affecting more than one thing.

### The arm invariant

> An arm of `Action` exists iff it has an executor no other arm has.

This deleted an arm. `Action::View(View::Down)` and a proposed `Action::Buffer(..)` both
routed to `self.focused()`, so they were one thing written twice; motions became
`Action::Buffer(BufferAction::Motion(..))`. `Tab` stays separate when it arrives because it routes
to the layout, not to a buffer. The full future set is `Program`, `Buffer`, `Pane`
(window-local settings), `Tab` (focus, resize, zoom), `App` (tabs), `Task` — six executors,
six arms, and no arm invented for a feature.

Each arm's payload is that executor's own commands, named `<Executor>Action`:
`Buffer(BufferAction)`, `Program(ProgramAction)`, `Task(TaskAction)`. The payload was once
called `Verb`, borrowed from vim's grammar, which matched neither the other two arms nor
vim — there a verb *takes* a motion, while `BufferAction` *contains* one.

### Each level owns its commands and binds them

One file per executor, holding that level's actions *and* the keys bound to them:

```text
input/buffer.rs    motions, and whatever a buffer kind adds   ← innermost
input/pane.rs      one pane, about its own view of a buffer
input/tab.rs       a tab, about its panes: focus, resize, zoom
input/view.rs      the whole view, about its tabs             ← outermost
input/program.rs   quit, suspend, redraw — below every level
input/task.rs      what leaves the crate
```

**Lookup walks that order, innermost first.** One mechanism, two jobs: it puts each level's
bindings where the level is, and it makes *shadowing* the answer to scoping. A buffer kind
that binds `<` claims it; anywhere else the same key falls through to the tab. Exactly how
Neovim's buffer-local mappings shadow global ones.

I argued against this at the time, on the grounds that per-level lists lose the ability to
scope a key — the explorer's `<` (collapse the sidebar, a *tab* action) would sit in the tab
list, live everywhere, colliding with a diff's `<` (narrow the split, a *buffer* action).
That was wrong: with innermost-first lookup the diff's `<` shadows it, and in the explorer,
which binds no `<`, the chain falls through. Both work, and no third concept is needed.

Two rules follow, and both are tested:

- **Exact shadowing across levels is legal** — it is the mechanism above.
- **A proper prefix anywhere in the chain is not.** `g` on a buffer would make `gg` on the
  tab unreachable in that buffer, silently and only there.

`Context` is what remains of the old design: it names which *buffer kind* has focus, and so
selects only the innermost list. Every level above binds the same keys whatever has focus.

### Two dividers, two owners

The rule is scale-free, and applying it twice settles a question that had been left open:

| divider | between | lowest container | owner |
|---|---|---|---|
| the `│` in a side-by-side diff | two **columns** | one buffer draws both | the **buffer** |
| a pane border | two **panes** | one tab holds both | the **tab** |

So `SideBySide` owns a `divider: u16` — the share of the width given to the original — and
`>`/`<` are `BufferAction::WidenOriginal`/`NarrowOriginal`. It sat on `Viewport` at first,
where its own comment admitted the problem: *"meaningless unless the buffer draws two"*. A
`Text` buffer had a column divider. Position is pane state because every buffer kind has a
position; a two-column ratio is not.

That also decides what was listed as open — whether the ratio is per-buffer or per-pane.
Per-buffer: two panes on one diff scroll together only if we want them to, but they drag
their dividers independently, because the divider is part of how the buffer draws itself.

The word is deliberately not *split*. In a Neovim-shaped model a split is what makes a new
pane, which arrives at S8; using it for a divider inside one buffer would collide exactly
when both exist. `Column`, `Pane` and `Side` are likewise kept apart: a column is a region
inside a buffer, a pane is a rectangle in a tab, and a `Side` is `Original` or `Modified` —
a *version*, not a place, since inline mode puts both in one column.

### What this deletes

- `Sides` from `ui`. Which kind of buffer to build is decided by the pipeline, the last
  thing that knows how many sides were read; `Sides` moved there, where it describes what
  was *read* rather than what is drawn.
- `Layout::Single(Side)`, and then `Columns::One(Side)` after it. A diff always has two
  columns — a file with one side is a `Text` buffer, not a degenerate diff — so `Frame`'s
  fields are no longer `Option`. The compiler found this: the variant became unconstructed
  the moment one-sided files stopped being diffs.
- `Runner::run(|diff| …)`, and with it every `<'a>` in `ui`. D26's note about the
  closure is superseded.

### Deliberately not built

`Layout` has one variant, `Full`. Every arrangement we know of — a diff alone, explorer
beside a diff, history beside a diff — is one pane or two. Helix's `Tree` is ~600 lines with
climb-and-descend directional focus and buys nothing until a third arrangement exists. The
seam is a single enum in one file; `Overlay` is uninhabited for the same reason, but its
routing exists now because event dispatch changes shape when the first overlay arrives, and
doing that once is cheaper than doing it twice.

## D28 — one vocabulary for a file, so its identity cannot degrade

**The problem.** Fourteen types touched "a file"; five of them claimed to *be* one, and the
file's identity got worse at every step:

```text
RelPath(String)                          typed
FileDiff { path, previous_path, kind }   typed, structured
"old.rs → new.rs   (added)"              a String — three facts fused
Status { path: &str }                    called path; is not one
```

The last step is the damage, and it was irreversible. The status line rendered the whole
string in the path's bold style — including `(added)`, which is not part of any path — and
could not shorten a long path, because nothing could find where the path ended.

**The cause was a rule we wrote ourselves.** `lint-arch` forbids `ui → vcs`, so `ui` could
not name `vcs::RelPath`. Identity was therefore re-declared, and then flattened to a
`String` to smuggle it across a boundary the lint enforces. The `String` was the smuggling;
the lost facts were the toll.

### Not another layer — a vocabulary

A layer would add a step to the flow. This adds none: `crates/file-types` is a leaf with no
dependencies that `vcs`, `codediff` and `ui` all name.

```text
RepoPath      where a file lives — both spellings, one constructor
File          which file this is: a version on each side, either absent
FileContent   what one version holds — text, a binary blob, or nothing
DiffVersion   which of the two: Original or Modified
```

Only `File` is new; the other three were moved from where one layer happened to own them.
`vcs::RelPath` gained the absolute form and became `RepoPath`, `vcs::Content` became
`FileContent`, and `align::Side` became `DiffVersion` — that one had never been about
pairing, and `ui` was reaching into `align` to say which column it was drawing.

### Everything a reader is told is derived

```rust
pub struct File {
    original: Option<RepoPath>,   // None = added
    modified: Option<RepoPath>,   // None = deleted
}
```

`is_renamed()`, `only()`, `previous_path()` and the `(added)`/`(deleted)` note are computed
from that pair, never stored beside it. A `kind` field could disagree with the paths; a
`label` field already did.

This is VSCode's `MultiDiffEditorItem`, which is a pair of `Option<URI>` and whose renderer
recomputes "renamed" at paint time from `modifiedUri.path !== originalUri.path`. Its label
port is typed `setUri(uri, options)` — a string label is impossible by type.

### What the research actually showed

Neither reference passes one object through. VSCode has **eight** types between `git status`
and pixels; `codediff.nvim` has **nine**. Both convert explicitly at every boundary. What
they do have is one identity token that survives unchanged — VSCode's `URI`, welded onto
`ITextModel.uri` so identity and content travel together; the plugin's `Path`, one type
carrying both spellings from a single constructor.

Both also have our bug. VSCode filed it as #110694 — *"the tab title … is too long:
`very/long/path/file1.js <-> very/long/path/file2.js`"* — and the fix works, in the
maintainer's own framing, precisely because it truncates the two paths **while they are
still separate values**. He conceded the limit of the flat `(name, description)` pair:
*"The ideal solution would be `labelA | descriptionA ↔ labelB | descriptionB` but that is a
lot more work."*

The plugin went further and lost the facts outright: `status` and `old_path` are consumed as
control flow when a diff is opened (`explorer/render.lua:257-509`) and never stored, so its
diff view cannot answer "is this a rename?" — `history/render.lua:309` has to re-derive it
from the tree node. It also fuses root, revision and path into a `codediff://` buffer name
that needs four regexes to reverse.

So we are not copying either. We keep all three facts structured to the renderer, which is
the step both of them skip.

### What it fixed on screen

The status line formats from structure, dropping parts in the order a reviewer can afford
to lose them — directory first, then the rename source, never the file name:

```text
70 cols   deep/nested/dir/demo.rs → deep/nested/dir/renamed.rs   1 change  1/3
34 cols   renamed.rs                                             1 change  1/3
```

A test asserts `(added)` no longer carries the path's style, and fails if the note is drawn
with `status_path` again.

### One more thing it settled

`ui::Text` was a **presentation mode named after a content type**, sitting beside
`SideBySide`, which is named after a layout — two different questions on one enum, one line
apart. It is now `SingleFile`, and the axis is uniform:

```rust
enum Buffer {
    SideBySide(..),   // two versions, two columns
    Inline(..),       // two versions, interleaved      (later)
    SingleFile(..),   // one version — from either mode
}
```

Both diff modes fall back to `SingleFile` when a file exists on one side, because there is
nothing to lay out against. Under the old name that had no obvious answer, and the plugin's
version of not-answering-it is four near-identical `show_*_file` wrappers funnelling into
one function.

## D29 — `vcs::Changes`, because nothing there diffs anything

`vcs` exposed a `Diff` trait in a `diff/` folder holding `FileDiff` and `DiffKind`. The name
was defended in the crate's README as *"what git and jj both call it"* — but it was borrowed
from the one git command this crate never runs. The three it does run are:

```text
files()          git status --porcelain=v2       what changed
before(file)     git cat-file --batch           one version
after(file)      std::fs::read                  the other
```

List, then fetch. Computing the difference between two versions happens two stages later, in
the C engine. `FileDiff` was likewise not a diff — it is a status entry with a `kind`.

So: `trait Changes`, `changes/`, `ChangedFile`, `ChangeType`, and the field is `change`
rather than `kind` — a `kind: ChangeType` would repeat the same mismatch one level down.
Each says what it is, and the
word "diff" is left to the four things that genuinely are one — `diff-types` (what a diff
is), `vscode-diff` (what computes one), `pipeline/diff.rs` (the stage that calls it) and
`ui::Diff` (what gets drawn).

`ChangeType`'s doc no longer has to explain why it is not called `Change`, because the
conflict was with the *trait* name and that is gone.

### Two scopes in one trait, worth knowing

`files()` is repository-scoped; `before`/`after` are file-scoped. That is why no single word
fit, and why the old name managed to describe neither. `Changes` names the listing, which is
what the trait is *for*; reading one file's versions is how you follow it up.

What flows downstream is always **one file**: a `ChangedFile` plus up to two
`FileContent`s. A file present on one side is the same shape with one `Absent`, which is
also what `File`'s `Option` pair says — so the single-file unit never needs a flag.

### Caught while doing it

Renaming `align::Side` to `DiffVersion` (D28) had been done with a blanket substitution, and
`vscode-diff` had its own private `Side` enum for error reporting. The substitution renamed
that too, producing a **second, unrelated `DiffVersion`** in a different crate — exactly the
duplication D28 existed to remove, introduced by the commit that removed it.

`vscode-diff` now names `file_types::DiffVersion`, and there is one definition in the
workspace. `DiffVersion` deliberately has no `Display`: it is a selector, and how to spell it
belongs to whatever is printing — an error says "original", a status line might say "before".

## D30 — the contract is the types, so the trait went

`vcs` exposed `trait Changes` with `files`, `before`, `after` and `repo`. Its stated job was
neutrality: *"no index, no `HEAD`, no blob and no object id, because a system need not have
any of them"*.

It was not doing that job. One implementor, zero generic uses, and every call site importing
it as `Changes as _` — the idiom for "I just want the methods in scope". An inherent `impl`
wearing a trait's clothes.

**The neutrality came from the types in its signatures, not from the trait.** `ChangedFile`,
`File`, `RepoPath`, `FileContent` are all in `file-types`, which `cargo xtask lint-arch`
forbids from naming `vcs`. A lint is not opt-in; a trait is — nothing stopped someone adding
a `Git` method returning an `Oid`, and the trait would not have objected.

Better still, the guard turned out to be structural. `vcs` depends on `file-types`, so an
edge back is a **dependency cycle**: cargo refuses it before any lint runs. The rule is not
merely enforced, it is unrepresentable.

**What checks a backend has met the contract is the pipeline that calls it**, and that is
the stricter test. A trait proves four methods exist with the right signatures; the pipeline
proves they are the methods actually *needed* and that their results compose. A backend
returning a `ChangedFile` the pipeline could not use would satisfy the trait and still not
build.

So `trait Changes` is gone, `Git`'s methods are inherent, and `vcs/src/changes/` with it.
A second backend earns a trait extracted from two real implementations rather than guessed
from one.

### What moved, and the rule that decided it

| | |
|---|---|
| **nouns** — `RepoPath`, `File`, `ChangeType`, `ChangedFile`, `FileContent`, `DiffVersion` | `file-types` |
| **verbs and failures** — `Git`, `Repo`, `Error` | `vcs` |

`Repo` stays because it is a property of the *repository*, not of a file: `control_dir` is
where git keeps its own state, which the S15 watcher needs and no file has. The root is
different — `RepoPath` carries both spellings, so `RepoPath::root()` recovers it by
stripping the relative tail off the absolute, with no IO and no way for the two to disagree.
That removed the last reason for `repo()` to be in the signatures.

### The thing I got wrong three times

Asked whether all of this could live in one crate, I twice answered with edits instead of an
answer, and twice defended the split on "who names it today". That test is a snapshot: it
would have kept `RepoPath` in `vcs` before `ui` existed. The rule that survives is **nouns
below, verbs above** — a crate every layer names can hold no capability, because a
capability needs an error type and an error type is a layer's own.

## Open questions

| # | question | needed by |
|---|---|---|
| 4 | binary / symlink / mode-change / submodule presentation | S8 — one-sided files are settled in [D23](#d23--a-file-with-only-one-side-is-shown-in-one-pane-not-diffed-against-nothing); binary is refused with a message |
| 5 | licensing and `ATTRIBUTION.md` — the C is VSCode-derived and vendors utf8proc | S1 |

*Question 1 (explorer grouping) is settled in [04-milestones.md](04-milestones.md): one list, worktree vs HEAD, conflicts marked but not resolvable.
Question 2 (syntax engine) is settled in [D11](#d11--syntax-highlighting-is-in-the-mvp-via-syntect).
Question 3 (inline mode) is settled in [D19](#d19--the-container-owns-the-row-index-so-scroll-sync-cannot-exist): out of the MVP, no auto-switch.*
