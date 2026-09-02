# 05 — Decisions

A log of decisions made during design. The purpose is to avoid relitigating them.
When a decision changes, edit it here and mark what superseded it.

---

## D1 — Why a rewrite rather than a port

The Neovim plugin (`codediff.nvim`, 20,634 lines of Lua) cannot be extracted
because its domain logic and its host are fused: 53 of 79 files call `vim.api`
directly, 19 `require` calls work around cycles in `explorer/render.lua` alone,
and there is no layer that can be lifted out intact.

Root causes: split by verb rather than noun, Lua's lack of enforced module
privacy, no type system, and no acyclicity check. These are all defaults in
Rust.

---

## D2 — Reuse the C engine, compiled from source

Copy `libvscode-diff` into `vendor/`, compile with `cc`, OpenMP disabled.

The prebuilt `.so` was rejected because it requires dynamic linking, which means
shipping three files, needing `libgomp`, and breaking `cargo install`. Upstream
issues #48 and #58 are both runtime linking failures of that `.so`.

The library is 12 C files, 7,538 lines, builds in 2.1 seconds, and produces a
446 KB static archive with no external dependencies.

---

## D3 — Copy the C source, do not submodule

`vendor/libvscode-diff/` is a copy from a pinned upstream tag. `cargo xtask
sync-c` refreshes it; `cargo xtask verify-c` detects drift in CI. Corrections
needed for the pinned VS Code build live as explicit patches under
`crates/vscode-diff-sys/patches/`. The build applies them to `OUT_DIR`, never to
the vendored tree, and fails when an upstream refresh makes a patch stop
applying.

Submodules are the norm for `-sys` crates, but the C rarely changes and
submodule friction (recursive clone, CI config, confusing errors) costs more
than it saves during fast iteration. Switch to a submodule when the C
stabilises or a third consumer appears.

---

## D4 — Crate boundaries as the architectural firewall

Ten+ crates with a strictly acyclic dependency graph, declared before logic is
written. Rust modules within a crate can reference each other freely, so module
structure alone enforces nothing. Crate boundaries enforce separation by making
cycles a compile error and providing real package-private visibility
(`pub(crate)`).

The critical missing edge is `ui → vcs`. Because that dependency is not
declared, a renderer that shells out to git is a compile error.

---

## D5 — Crate naming

No `codediff-` prefix. Named after what the crate contains, never after a layer.
`core`, `common`, `utils` and `model` are banned because they have no admission
criterion — "does this belong in `align`?" has an answer; "does this belong in
`core`?" is always yes.

---

## D6 — Presentation is fast; data crosses threads

Keys that change what is on screen (scrolling, cursor, folds, focus) are handled
synchronously inside `ui` — sub-millisecond, no channel. Only file selection and
file reading cross to worker threads (`pipeline`, `syntax`).

Preserving cursor position across a file reload is done by keying position to
stable identity (`path`, `HunkId`) rather than to row indices.

---

## D7 — `HunkId` is a content hash

Hunks are identified by a hash of their content, not by index. Agents rewrite
files constantly; line numbers move while hunk content often does not.
Content-hash identity means cursor and review state survive a refresh, and "what
changed since I last looked" is set arithmetic.

---

## D8 — No async runtime

`std::sync::mpsc` channels and `std::thread`. No tokio, no rayon.

No network IO, bounded concurrency, and every operation is either a blocking
subprocess or CPU-bound. If an agent backend later needs tokio, it lives inside
that crate with a bridge to the sync channel.

---

## D9 — A deliberately thin motion set

`j k h l Ctrl-D Ctrl-U gg G ]c [c Tab Enter / n N q ?` and counts. Nothing
else. Additional motions added on demand from evidence.

---

## D10 — `git status --porcelain=v2 -z`

Porcelain v2 with NUL separators. v1 requires hand-parsing `old -> new` rename
arrows and quoted paths, which breaks on spaces and unicode. v2 with `-z`
eliminates that class of bug. `--no-optional-locks` on every invocation to avoid
index.lock contention.

Blob reads go through a long-lived `git cat-file --batch` process rather than
spawning one `git show` per file.

---

## D11 — Syntax highlighting via both syntect and tree-sitter

Included in the MVP. Two engines: a parser (tree-sitter, 25 languages) for
accuracy, and a matcher (syntect + two-face, ~183 languages) for coverage. One
engine is picked per file — parse where we have a grammar, match where we don't.

syntect was chosen over tree-sitter-only because ~200 languages work with no
per-language effort. tree-sitter-only was rejected because each language needs a
grammar crate, a `highlights.scm`, and a mapping. Both are used because the
matcher leaves ~35% of identifiers unscoped where the parser leaves ~21%.

The trigger to drop syntect entirely: when all languages a reviewer encounters
have maintained grammars.

---

## D12 — Future features tested against the architecture

Six planned agent-review features were checked against the crate structure. Five
require no structural change; one (agent comments inline against hunks) requires
`ui`'s row type to be an enum with room for non-diff rows (not yet built).

---

## D13 — jj support

Not yet a decision. jj auto-snapshots the working copy on every operation, so
its operation log answers "what did the agent change since T" for free. This is
the strongest argument for the `vcs` layer being backend-agnostic.

---

## D14 — Split `vscode-diff-sys` and `vscode-diff`

Standard Rust `-sys` convention. `vscode-diff-sys` holds raw FFI + build script
(~150 lines of unsafe). `vscode-diff` holds the safe API. The seven other crates
carry `#![forbid(unsafe_code)]`.

The unsafe surface is countable: one crate, ~150 lines of declarations, ~40
lines of pointer-to-owned conversion in `convert.rs`. No C pointer ever escapes
into application types.

---

## D15 — File watcher: `notify`, not Watchman

**Not yet built.**

`notify` + `notify-debouncer-full` as the default. Watchman rejected because it
is a daemon the user must install (contradicts single-binary), its Rust client is
stale, it requires tokio, and modern Linux `max_user_watches` defaults (524,288)
eliminate the scale problem it solves.

---

## D16 — Watcher design

**Not yet built.**

Key decisions from upstream production failures (codediff.nvim #480, #482):

- Watch both worktree and `.git/`
- Filter lock files by destination, not by path substring
- Prevent self-triggering by suppressing refresh during our own git calls
- Watch directories, not individual files
- Watch `.git/` non-recursively (avoid `.git/objects/` event storm)
- No routine polling — zero git subprocesses while idle
- Debounce ~50 ms
- Respect `.gitignore` via the `ignore` crate

---

## D17 — Syntax highlighting is its own crate

Highlighting lives in `crates/syntax/`, not inside `ui`. Its public interface
never names a syntax engine.

The interface is whole-file-in, spans-out:

```rust
pub fn spans(text: &[&str], lang: Language) -> Vec<Vec<Span>>
```

This hides the difference between syntect (stateful, line-by-line) and
tree-sitter (parse whole file, then query). Both engines map into a shared
`Group` enum (~31 values like `Keyword`, `Type`, `Function`, `String`). `ui`
maps `Group → Color`.

The engine is swappable per-language because no engine type appears in any
public signature. `cargo xtask lint-arch` fails if `syntect::` or
`tree_sitter::` appears outside `crates/syntax/src/engine/`.

---

## D18 — `align` matches VSCode's model: nothing stored per row

`Alignment` borrows the `LinesDiff` and both files and computes every answer on
demand. It stores no rows and no text. A change of `original 2..3, modified 2..2`
already says "one original line, no modified line" — that is the filler.

This is VSCode's design: its `DiffState` is a thin wrapper over the engine
result. Ours drops the two pixel-height fields it carries for line wrapping.

The old design (`AlignedDoc { rows: Vec<Row> }`) was replaced because it grew
with file size rather than edit count, could disagree with the diff, and baked
left/right layout into the model.

---

## D19 — One row index, no scroll sync

A single `Viewport` owns one vertical position. Side-by-side draws row *n* in
both panes — left takes `row.original`, right takes `row.modified`. They cannot
drift.

The plugin spent 415 lines on scroll synchronisation fighting Neovim's separate
`topline`/`topfill`. Here a row index means the same thing on both sides.

Wrapping (when built) will make pairing depend on pane width, so wrap-aware
alignment will live in `ui`, not `align` — the same split VSCode makes.

---

## D20 — Type names mirror the C header

Our Rust types mirror `vendor/libvscode-diff/include/types.h`, which mirrors
VSCode. `LinesDiff`, `DetailedLineRangeMapping`, `MovedText`, `RangeMapping`,
`LineRange`, `CharRange` — all match the header. The cost is verbosity; the
benefit is that VSCode's source directly explains our behaviour.

---

## D21 — `vcs` runs `git` rather than linking a git library

`gix` and `git2` exist but were not used. The reason is not speed (4.5 ms for
`git status` on 340 files). The reason is that git's own binary honours the
user's config, `.gitignore`, sparse checkout and clean filters — rules that
decide which files appear at all. A reimplementation that differs anywhere shows
the wrong list.

Two layers: a `git/` module that runs commands and parses output in git's own
words, and `repository/` that translates into our standard types.

Blobs come from one long-lived `cat-file --batch` child, not one process spawn
per file.

---

## D22 — Catppuccin by arithmetic

The diff backgrounds are derived from the palette by blending:

```
out = round(alpha × accent + (1 − alpha) × base)
```

So a flavour is 26 palette colours plus one derivation function. A test asserts
the derivation still reproduces Catppuccin's published values.

A `basic` theme family exists for terminals without 24-bit colour. It uses
`Color::Reset` for backgrounds and the 256-colour cube for diff highlights. A
test asserts it never emits a 24-bit colour.

Detection: `COLORTERM` for 24-bit support, overridable with `--theme`.

---

## D23 — A one-sided file is shown in one pane

An added, untracked or deleted file is not diffed against nothing. It gets one
pane at full width. Nothing is highlighted because nothing changed relative to
anything.

VSCode does the same: `getLeftResource` returns a URI only for modified/renamed
files; added and deleted fall through to a single editor.

An empty tracked file (zero bytes) still gets a two-pane diff — "absent" is
different from "empty".

---

## D24 — Keys resolve to three kinds of command

| kind | answered by | latency |
|---|---|---|
| Buffer/View action | `ui`, this frame | µs |
| Program action | terminal owner (quit, suspend) | µs |
| Task | composition root, off-thread | ms |

Resolving (turning keys into a command) and dispatching (sending it to the right
executor) are separate. The resolver is a pure function of its own state and one
key — no clock, no IO.

The binding table is `const` data (using `crokey`'s `key!()` macro), so it can
be printed into a help screen and walked by tests. No binding may be a proper
prefix of another — this removes the need for a timeout.

---

## D25 — `diff-types` is separate from the engine

`align` needs to name a diff result but must not depend on the C engine (or it
would require a C toolchain to build). The six diff structs live in `diff-types`
— no dependencies, no build script. `vscode-diff` depends on it and re-exports.

Result: `cargo build -p align` takes 0.7s, not 4.2s.

---

## D26 — One pipeline, sequential stages

The file pipeline: resolve → read both sides → diff → align → return. Five
stages, five files, in `pipeline/`. The pipeline lives in `codediff` (now
`pipeline` crate) because it is the only place that names `vcs`, `vscode-diff`
and `align` together.

`ui` defines what it consumes (`pipeline::file::DiffContent`). The consumer
defining its own input keeps the dependency graph acyclic.

---

## D27 — View → Tab → Pane → Buffer

Four levels, each containing the next. Buffers live in `View` referenced by
`BufferId`, never by `&mut` reference (that would make the structure
self-referential).

Position lives on the pane, not the buffer — two panes over one buffer scroll
independently.

Side-by-side and inline are different buffer kinds, not one buffer with a flag.
They emit different row sequences, so "row 40" would mean different things with
a flag.

`Alignment` owns its two files (no lifetime parameter). The earlier borrowed
version forced a closure-based API and propagated `<'a>` through every type in
`ui`.

---

## D28 — One vocabulary for a file: `file-types`

`crates/file-types` is a leaf with no dependencies. It defines `RepoPath`,
`ChangedFile`, `FileContent`, `DiffVersion`, `DiffType`. Every layer — `vcs`,
`pipeline`, `ui` — names these same types, so a file's identity never degrades
across boundaries.

The status line formats from structure (dropping directory first, then rename
source, never the file name) rather than from a pre-rendered string.

---

## D29 — `vcs::Changes` → `vcs::Repository`

The crate runs `git status`, `git cat-file`, `git diff --numstat`. It does not
compute diffs. So the types are `Repository`, `ChangedFile`, `ChangeType` — not
`Diff` or `FileDiff`, which describe what happens two stages later in the
pipeline.

---

## D30 — No trait for the VCS backend

One implementor (git), zero generic uses, every call site importing the trait
just for method resolution. The neutrality comes from the types in the
signatures (`ChangedFile`, `FileContent` — all in `file-types`), not from a
trait. A trait is added when a second backend exists to extract it from.

---

## D31 — `align` owns both layouts

`align` exposes `DiffType::SideBySide` and `DiffType::Inline`. The difference
is one line of arithmetic:

```rust
let height = original.len().max(modified.len());   // side by side
let height = original.len()  +  modified.len();    // inline
```

Everything else (fillers, change types, inner-change spans) is shared. Both
yield the same `ViewLine` type.

---

## D32 — One word per idea

A view line is `ViewLine`, never `Row`. A classifying enum uses the suffix
`Type`, never `Kind`. `cargo xtask lint-arch` refuses `Kind`, `Data`, `Info`,
`Manager`, `Helper`, `Handler` in any type we declare.

---

## D33 — `DiffType` defined once

`DiffType { SideBySide, Inline, Single }` is defined once in `file-types`.
Everything else holds a value of it rather than restating the variants.

---

## D34 — Render bricks don't know the model

`ui/src/render/` (cells, gutter, column, line, layout) puts characters on a
grid. It may not name `crate::view`. `ui/src/draw/` composes those bricks into
what a buffer type looks like. Enforced by `lint-arch`.

---

## D35 — The divider belongs to `SideBySide`

`BufferType` has three variants: `SideBySide { diff, divider }`,
`Inline { diff }`, `SingleFile { file, lines }`. The divider does not survive a
switch to inline and back — inline has no columns to divide.

---

## D36 — What to take from VS Code, delta, and bat

A feature is included unless it exists only to support editing. The audit is in
`crates/syntax/README.md`.

Key findings: ten abstract token kinds is lossy (a real theme needs ~25
colours); delta's per-hunk reset is a bug we avoid by reading whole files; bat's
tab/bidi/long-line handling is a security control we adopt.

---

## D37 — A span carries a pen, not a colour

`syntax::Span` reports `Pen(u16)` — an index into a table `ui` supplies. This
means changing theme invalidates no span, the `basic` theme can use indexed
colours safely, and the scope table is one shared constant rather than one per
theme.

---

## D38 — Frame-based colouring budget (superseded)

> Superseded by D41. The reasoning explains why; the answer (thread) replaced it.

Slicing work against frames cannot survive an engine whose smallest unit is
indivisible, and tree-sitter has two of those (`Query::new`, `highlight`).

---

## D39 — Two engines, one file each

The parser (tree-sitter) colours 79% of identifiers on this codebase; the
matcher (syntect) colours 65%. Both resolve to the same `Group` enum through
shared `Pen` indices, so a file looks the same regardless of which engine
coloured it.

The binary cost: 3.6 MB → 40 MB (36 MB is `.rodata` — generated parse tables,
memory-mapped, only the used language is paged in).

---

## D40 — Lazy language preparation (superseded)

> Superseded by D41. Kept because it has the measurement that forced the thread.

`Query::new` takes 16 ms for Rust, 247 ms for Haskell. Doing that on the
drawing thread was a visible stall. The idle-pass workaround bought 12 ms text
at the price of a 186 ms keypress — the same bug moved.

---

## D41 — Colouring on a worker thread

A worker thread colours; the interface asks and installs. `std::thread` and
`std::sync::mpsc`. The interface never waits for a colour.

What forced it: tree-sitter's `Query::new` (up to 247 ms) and `highlight` (no
range API — whole file every call) are both indivisible. No frame-slicing scheme
can work.

Measured: worst keypress dropped from 186 ms to 13 ms.

The worker holds the engine state (not `Send`). Text goes in, spans come back,
both plain data.

---

## D42 — The interface keeps spans; the worker keeps its place

Spans live on the interface thread (drawing needs them immediately). The engine
bookmark lives on the worker thread (it holds a non-`Send` pointer). Nothing is
stored twice.

Eviction is LRU by line count (~800k lines budget ≈ 64 MB). The file on screen
is never evicted. A stale request (sent before the last answer arrived) is
dropped and re-asked with a current offset.

Read-ahead: 2,000 lines beyond the viewport, so scrolling finds colours already
there.

---

## D43 — Engine vocabularies live in `syntax`, not the theme

The matcher's `comment.line.double-slash.rust` and the parser's `comment` both
map to `Group::Comment`. Those mapping tables live in `syntax/src/engine/`,
beside the engine whose words they hold.

The theme never sees engine-specific strings. Adding a theme is 31 colours, not
78 scope rules. `lint-arch` refuses `syntax::engine` imports from `ui`.

Renamed: `Token` → `Group`, following Vim's `:help group-name`.

---

## D44 — File versions named the way git names them

Every version is keyed by its git name: `b87b24c…:src/main.rs`,
`:0:src/main.rs` (index), `:2:path` (ours in a merge), or `worktree:path`.

This prevents a bug where two comparisons of the same path (staged vs HEAD,
worktree vs HEAD) would collide in the colour store. The key is a string that
cannot be confused because a resolved commit id is 40 hex characters.

`HEAD` is resolved to an id once at startup, so a commit made during the review
cannot split the before-side naming.

---

## D45 — A change is split before fillers are placed

Where the engine found character-level matches within a change, the matching
lines are pulled level and fillers go around them. This matches how codediff.nvim
and VSCode place fillers.

Checked against the plugin: 168 cases, filler/mark parity on all of them once
the phantom-trailing-line difference is accounted for.

---

## D46 — Known parity differences with the plugin

Two measured differences, understood and left standing:

1. We keep the empty piece after a trailing newline (3 lines vs plugin's 2).
2. A character-level highlight that reaches past the last line — we show it, the
   plugin drops it as a stale-diff defence.

---

## D47 — A staged-then-edited file is listed twice

Git reports `MM` as two comparisons of one path: index vs commit, and worktree
vs index. They are two rows in the explorer, each with its own diff. This
matches VS Code's SCM view.

---

## D48 — The list starts on the first file

The cursor starts on the first file, not on the group heading above it. A
heading folds but cannot be opened, so starting there would make the first
keypress do nothing.

---

## D49 — A binding's list and its executor differ

Where a key is defined (which level's binding table) and where it is executed
(which level handles it) are separate questions. `>` is bound by the focused
buffer but executed by the tab (it affects two panes). `t` is bound by the view
but executed on whichever pane shows a diff.

---

## D50 — Rows measured in cells, cut in cells

A CJK character is one `char` but two terminal columns. Measuring with
`chars().count()` causes misalignment. Everything that truncates or measures a
row for the terminal uses `line-index` (cell/column width), not character count.

---

## D51 — Nothing is kept when a file is opened twice

Three of the four revisions (worktree, index, conflict stage) are mutable. A
cache keyed by revision + path cannot tell fresh from stale. So nothing is
cached — reading two versions and pairing them takes milliseconds.

---

## D52 — The file list has its own colour table

The explorer's colours are separate from the diff's. `theme::Tree` colours rows
that nest (headings, directories, guides). `theme::Change` colours a file by
what happened to it (added, modified, deleted, etc.) — reusable anywhere a
changed file is named.

---

## D53 — A viewport belongs to a pane

A viewport is a position in a buffer. Repointing a pane at a new buffer while
keeping its viewport would show the new file at the old file's scroll offset. So
opening a file installs a fresh pane.

---

## D54 — Anchor by name, not by row number

Toggling view mode renumbers rows. The cursor is anchored by (section, path),
looked up after the rebuild. Same principle as carrying a file line across a
layout toggle.

---

## D55 — A key must not be silently dead

`t` (toggle layout) was bound at the view level but acted on the focused buffer.
When the list had focus, it did nothing silently. Now it acts on whichever pane
shows a diff, regardless of focus.

---

## D56 — Git switches are forced

Every git command forces the flags it depends on. A reader's `diff.renames=false`
must not make the status say "rename" while the line counts say "new file."

Also handled: unborn HEAD (use the empty tree), symlinks (use `read_link` to
match git), submodules (content is a commit id, shown as unreadable).

---

## D57 — A group is a revision pair

"Staged Changes" is not a category — it is the name for comparing the index
against a commit. A group is `{ name, revs, files }`. Adding a new comparison
mode touches one file (`pipeline/list`), not the explorer or the drawing code.

---

## D58 — The list is the search

The file pipeline no longer searches for a file by path. A `ChangedFile` carries
its revisions, so the row *is* the request. `codediff <path>` is a pathspec
filter on the list, not a different mode — same code path either way.

---

## D59 — The interface asks, never computes

Opening a file runs on a worker thread (measured: up to 1057 ms for a 50k-line
file). `ui` sends a `ChangedFile` to the file worker, draws whatever it already
has, and installs the response when it arrives. No callback, no `Flow::Task`.

---

## D60 — `DiffType` instead of `Option<DiffLayout>`

One enum in `file-types`: `DiffType { SideBySide, Inline, Single }`. The old
code spelled the same fork five different ways, four of them using `None` to mean
"single file." A third answer (the explorer) is not an absent one.

---

## D62 — No revision arguments on the command line

`codediff` takes a path and a theme. No `--rev`, `--staged`. The model is
lazygit's: open with one word, change what you're comparing from inside the
review.

---

## D63 — The rule is about blocking, not about crate names

The old rule (`ui` may not name `vcs`) was false — `ui → pipeline → vcs`
already exists. The real rule: nothing reached from inside the event loop may
block. `lint-arch` checks four directories (`input`, `draw`, `render`, `view`)
for `std::fs`, `std::process`, `vcs::`, `recv()`, `join()`.

`try_recv` is allowed — it is how the loop collects answers without waiting.

---

## D64 — `ui::start` owns setup; `main` is minimal

`main` parses arguments and calls `ui::start(cwd, paths, theme)`. Everything
else (opening git, reading the file list, constructing the session) lives inside
`ui::start`, which runs before the terminal is opened and may block.

---

## D65 — The model reports facts; only the terminal picks characters

`align` reports that a view line is a gap. It never says a gap is drawn `╱`.
`explorer` reports that a file was renamed. It never builds `"← old-name.rs"`.

The split: `explorer` emits structured `Content` (heading/directory/file with
fields). `ui/draw/` turns those into characters and colours.

---

## D66 — Colour tables named for what they colour

`theme::Tree` — rows that nest (heading, guide, directory, name, count).
`theme::Change` — what happened to a file (indexed by `ChangeType`).

Separated because `Change` colours are reusable anywhere a changed file is
named (status line, tabs), not just in the explorer.

---

## D67 — The backend runs commands; the layer above makes a review

`vcs` exports `Repository` — open, list changes, count changes, read a version.
`git/` is private. Two layers:

- `git/` runs one command, parses its output in git's own vocabulary
- `repository/` translates into the standard types (`ChangedFile`, etc.)

The list pipeline collapsed from two stages to one once planning logic moved
into `git/`.

---

## D68 — A feature must not add a file to `render`

Every file in `render/` arrived with the terminal itself. Files that arrive with
a feature belong beside that feature. `list.rs` and `fit.rs` were moved out of
`render/` into `draw/buffer/explorer/` when it was clear they were the file list
wearing a brick's name.

The `explorer` crate is now separate from `ui`. A tree with nesting and folds is
the model; rows with text and colour are the drawing. What both use is
`line-index` for column measurement.

---

## D69 — A heading is what an arrangement sits under

A heading is not part of a tree or a flat list — it is what either arrangement
sits under. `Explorer` owns the groups, headings, counts and heading folds. A
`Style` (tree or list) is handed one group's files and produces lines without
knowing what a heading is.

Sort order matches VS Code: shallower paths first, numeric comparison for digit
runs. The sort key is built once per path (not per comparison), making 20k paths
sort in 1.2 ms vs 81 ms with inline case-folding.

---

## Open questions

| # | question |
|---|---|
| 4 | binary / symlink / mode-change / submodule presentation |
| 5 | licensing and `ATTRIBUTION.md` — the C is VSCode-derived and vendors utf8proc |
