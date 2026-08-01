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

The critical missing edge is `display → vcs`. Because that dependency is not declared, a
renderer that shells out to git is a compile error — preventing by construction the failure
that produced a 674-line `explorer/render.lua`.

---

## D5 — Crate naming

**Decision.** No `codediff-` prefix. Crates named after the thing they contain, never after
a layer.

```
vscode-diff-sys  vscode-diff  metrics  syntax  align  explorer  vcs  runtime  display  codediff
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
| agent comments displayed inline against hunks | **required a decision**: `display`'s `VisualRow` must be an enum wrapping `align::Row` with room for non-diff rows, and projections must take a context struct. Due at S7 |
| "what changed since I last looked" | **free** — falls out of `HunkId` being a content hash |
| MCP server so the agent queries the diff | **clean** — everything except `display` and `codediff` is already headless |
| base revision = "when the agent started" | **clean** — `ContentSource::Snapshot` reserved; free if the repo is jj-backed |
| agent writes files while you review | **clean** — read-only means *codediff* never writes, not that nothing changes; the watcher already covers it |

### Risks that would force a genuine rewrite

| risk | insurance taken now |
|---|---|
| review state becomes primary and diff secondary, making `align` the wrong centre | watch for annotations gaining more fields than hunks |
| multiple simultaneous diffs (three-way, tabs, comparing two revisions) | `AppState.docs: HashMap<DocId, Document>` from the start, with one entry |
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

**Decision.** Highlighting lives in `crates/syntax/`, not as a module inside `display`, and
its public interface never names a syntax engine.

**Why not a module in `display`.**

1. It is not a rendering concern. Highlighting is text *analysis* — text plus language
   produces spans; rendering *consumes* spans. Conflating them is the same category error
   that produced a 674-line `explorer/render.lua`.
2. `display` would carry a heavy dependency (syntect and two-face, or tree-sitter and N
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

Both engines map *into* it. `display`'s theme maps `Class → Style`. The split is clean:
**`syntax` says what something is, `display` says how it looks.**

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
stored field: `runtime` owns a `Document` (the texts, their line vectors and the `LinesDiff`) and
constructs an `Alignment` where one is needed. Storing the borrowing type would make
`AppState` self-referential.

**Cost accepted.** Locating a row is a walk over the changes rather than an array index.
Rendering asks for consecutive rows, so this is one pass per frame — seven iterations on
`comprehensive_move`. A prefix sum would make it `O(log n)`, is derivable from the changes,
and can be added without changing the interface if a file with thousands of changes ever
makes a scrollbar drag feel slow.

---

## D19 — the container owns the row index, so scroll sync cannot exist

**Decided at S4, before building `display`.** A `View` owns one vertical position; a `Pane`
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

**Alignment with heights belongs in `display`, not `align`.** Once wrapping exists a line is
no longer one row, so pairing depends on pane width — which is why VSCode computes
`ILineRangeAlignment` in its *view* while `DiffState` stays width-independent. `align` keeps
the width-independent pairing; `display` computes row counts at the current width and pads
after a range on the shorter side. `LineRangeAlignment { original, modified, original_rows,
modified_rows }` is the right name for that, in `display`, at S10a.

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
kind of wrong a review tool cannot afford. `Vcs` is a trait, so this is reversible, and a
future `jj` backend needs one anyway.

**Two layers, each in its own language.** The `Vcs` trait is in the reviewer's terms —
`changed_files`, `before(file)`, `after(file)` — and names no git concept, because a system
need not have one: jj has no index and no `HEAD`. Underneath, `git/` keeps every git word,
with modules named for the commands they run, and `git::to_change` is the single point the
two vocabularies meet.

Forcing one vocabulary on both would go wrong in either direction — inventing fake-neutral
names for git things, or making a jj backend pretend it has a staging area.

**Capabilities that only some systems have get their own traits.** `Staging` and `History`
would sit beside `Vcs`, so a backend lacking one fails to compile rather than answering
"unsupported" at runtime. Only `Vcs` exists today. Note that staging is *not* excluded for
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

## Open questions

| # | question | needed by |
|---|---|---|
| 4 | binary / symlink / mode-change / submodule presentation | S6 |
| 5 | licensing and `ATTRIBUTION.md` — the C is VSCode-derived and vendors utf8proc | S1 |

*Question 1 (explorer grouping) is settled in [04-milestones.md](04-milestones.md): one list, worktree vs HEAD, conflicts marked but not resolvable.
Question 2 (syntax engine) is settled in [D11](#d11--syntax-highlighting-is-in-the-mvp-via-syntect).
Question 3 (inline mode) is settled in [D19](#d19--the-container-owns-the-row-index-so-scroll-sync-cannot-exist): out of the MVP, no auto-switch.*
