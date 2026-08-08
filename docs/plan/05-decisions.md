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
sync-c` refreshes it; `cargo xtask verify-c` detects drift in CI.

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
| once, before the terminal is opened | **yes** — there is nothing to stay responsive with |
| inside the loop, on a key or a frame | **no** |

That is the same reasoning that keeps the file list synchronous while a diff
runs on a thread, and it is the principle D59 should have written down instead
of a crate name.

So the text rules are gone and `NON_BLOCKING_DIRS` replaces them: four
directories — `input`, `draw`, `render`, `view` — reached only from inside the
loop, in which nothing may name `std::fs`, `std::process`, `std::net`, `vcs::`,
`vscode_diff::`, `recv()` or `join()`. `try_recv` is deliberately allowed: it
is how the loop collects an answer without waiting.

**`app.rs` is not covered**, because it holds the loop *and* the startup that
precedes it, and a directory rule cannot separate them. Splitting those into
different files is what would let the rule reach it.

The manifest edges stay, with honest reasons: `ui` still must not name `vcs`
directly, not because it could reach git — it can — but because git is reached
through `pipeline`, which owns the thread it runs on. One seam, not two.

## D64 — the interface starts itself; the binary hands it a place to look

`main` used to run the review: resolve the theme, open the repository, build
the request, run the list pipeline, refuse an empty one, construct a session,
ask for the first file, and start the loop. Eight steps, of which two were the
binary's and six were the interface's.

It is `ui::start` now, and `main` is:

```rust
let cwd = std::env::current_dir()?;
ui::start(cwd, path.into_iter().collect(), cli.theme.as_deref())
```

**A file of its own, not part of `app.rs`**, because the two obey opposite
rules. `start` runs once, before the terminal is opened, so it may block —
there is nothing to stay responsive with, which is why the file list is read
rather than asked for. The loop may not block at all. [D63](#d63) could not
reach `app.rs` while it held both; with the split it does, as a file rule.

**And `vcs` was never needed.** `main` opened the repository to put its root in
the request. But `list::resolver` opens git again from that path, and every
path it builds comes from the root git *discovered*, not from the one it was
handed. So the request carries a place to start looking, `git rev-parse
--show-toplevel` does the rest, and `ui` names no version control at all.

`main.rs` fell from 85 lines to 41, and stopped naming `vcs`, `explorer` and
`pipeline`. It parses arguments and picks a subcommand; everything else it used
to do belonged to somebody else.

`ui` gained `anyhow`, because "nothing has changed here" is not an IO error and
`run` returns `std::io::Result`. Every crate here is `publish = false`, so the
usual objection to `anyhow` in a library does not apply.

## D65 — a model reports facts; only a terminal picks characters

**Superseded in part by [D68](#d68)**: the split of facts from characters holds,
but `render` was the wrong side of the line to put `list.rs` and `fit.rs` on.

`align` reports that a view line is a gap. It never says a gap is drawn `╱` —
that word lives in `ui::render::column`, beside the theme that colours it.

`explorer` did the opposite. `rows.rs` built `"│ └ "`, `"▾ "`, `"+4"`, `" -1"`,
`" (2 · "` and `"M"`, and handed over finished strings. Its own admission
criterion, at the top of `lib.rs`, said *"never how they look"*. Five places in
one file broke it.

So the two halves of the screen were built by opposite rules, and only one
followed the rule the code claimed.

**What it cost, measured.** A `Region` — text plus a droppable priority — is a
general idea: *here are the pieces of a row, drop the cheap ones and cut the
longest*. But it was `explorer::Region`, so the one function built on it took a
slice of them, and nothing that was not a file list could call it without
pretending to be one.

The status line needs exactly that rule — it drops a directory, then a rename,
to keep the file name — and `draw/status.rs::name()` writes it again by hand in
forty lines. The two copies do not agree: `name()` counts `chars()` where
`fit` counts columns, so it measures a Japanese path at two-thirds of the
columns it takes. Driven through a pty with two paths **eighteen terminal
columns wide**, at width 36:

```text
abcdef/filename.rs  →  " filename.rs      3 changes   1/100 "
なまえ/ファイル.rs    →  " なまえ/ファイル.rs                 "
```

The ASCII path drops its directory and keeps the position. The CJK path keeps
the directory and silently loses `3 changes 1/100`.

**That is left as it is**, and recorded as [B9](06-known-bugs.md). It is a
behaviour change, and this is a refactor: `fit` is reachable from the status
line now, which is what this decision is about. Making it call it is a commit
of its own.

One rule, written twice, and the copy that could not reuse the original is the
one that is wrong.

**The split.** A `Row` is now three facts — which node, where it sits, what it
is:

```rust,ignore
Row { node, guides: Option<Guides>, content: Content }
Content::{ Heading { title, files, stats },
           Directory { name, open },
           File { name, moved_from, stats, change } }
```

`guides` is `Option` rather than an empty `Vec`: a heading is what the tree
hangs from rather than a line in it, and "no indent to describe" is a different
statement from "at the top level". Inside it, `ancestors: Vec<bool>` says for
each level above whether that ancestor was the last of its siblings — the exact
question "does this column need a guide, or blank space".

And in `ui`, two files where there was one:

| file | what it is |
|---|---|
| `render/fit.rs` | a `Piece` is text, a style and a priority. Knows nothing else. |
| `render/list.rs` | facts plus a theme, in text and colour. |

`render/list.rs` is the same brick as `render/line.rs`: each takes what its own
crate reports, adds a theme, and answers in text and colour. `line` is the
diff's, `list` is the file list's. Neither decides what fits.

**Both names were wrong before this.** The file was `render/explorer.rs` —
named for the buffer type that called it, while every other brick is named for
the terminal thing it makes. `render/mod.rs` listed five bricks when there were
six, and never listed that one: the list is written in terminal words and the
name did not fit it. `row.rs` was considered and refused — `cells.rs` already
calls one row of the grid `row: Rect`, and this codebase has already paid for
that collision once, when `align`'s `Row` became `ViewLine`. `list` was already
the word in `ui` for the thing on screen, which is what the theme tables are
named after — and [D66](#d66) then split those tables by the same test this
decision applies to `render`.

**What is checked where.** `explorer/tests/tree.rs` spells the facts with a
helper of its own, so its 360 lines of assertions about the *shape* of the tree
survive unchanged. The characters are asserted in `ui/tests/explorer_rows.rs`,
against a real screen — where they can be wrong.

Sabotage: a changed guide, a changed fold triangle and a changed status letter
each fail three or more tests. Bold on a heading failed nothing at all, having
been moved on trust; `a_heading_and_a_status_letter_are_bold_in_every_theme`
now covers it.

## D66 — a colour table is named for what it colours, not for who draws it

`theme::List` held fourteen colours, and they answered two different questions.

Five need rows that nest to mean anything: `heading`, `marker`, `directory`,
`name`, `count`. An indent guide is nothing where nothing indents.

The other nine are about a **file**, and mean the same wherever one is named:
six for what happened to it — added, modified, deleted, renamed, untracked,
conflicted — and `added`/`removed` for the lines it gained and lost. A tab of
open files would want them. So would a header over a diff, or the bottom row.
None of those is a list, and none would think to look inside a table named for
one.

The table's own doc said as much without noticing: *"the letters follow the
diff's own colours where they exist — green for what arrived, red for what
went, so the list and the file beside it agree about what green means."* That
is a claim about the whole screen, written inside one buffer type's table.

So:

| table | what it colours | indexed by |
|---|---|---|
| `theme::Tree` | a tree drawn in rows | what a row *is* |
| `theme::Change` | a file that changed | `file_types::ChangeType` |

`Change::of(ChangeType)` is on the table rather than at each caller, so a
seventh kind of change is a field and one arm rather than a search for
everywhere six were spelled out. `render/list.rs` had that `match` inline and
was the only place it existed; now it has none.

**Two fields were renamed on the way, both because they collided.**

`List::moved` was grey, and meant *where a file came from*. `Theme::moved` is
faint blue, and means *a block the engine judged to have moved within a file*.
One word, two meanings, one struct apart. It is `Tree::previous` now, which is
what the row beside it says: `← old-name.rs`.

`List::new_file` was the odd one of the six — five named for a `ChangeType`
variant and one not, because `added` was taken by the line count in the same
struct. With the two tables apart, `Change::added` is the change and
`Change::gained`/`Change::lost` are the counts, so nothing is named around a
clash that no longer exists.

**Why not `theme/explorer.rs`.** The theme files are named for what a reader
sees — `code`, `colour`, `catppuccin` — never for the code that draws it;
`code.rs` is not called `syntax.rs`. And `Explorer` already means three things
in `ui`: the crate, the buffer, and the `BufferType` variant. A fourth would be
the collision [D65](#d65) had just removed one layer down.

The split is what makes the question moot. `Change` is not the explorer's, and
`Tree` is named for the shape it colours rather than the buffer that has one.

## D67 — the backend runs commands; the layer above turns them into a review

`vcs` exported `Git` — eleven methods — and also `pub mod git`, so everything
under it was reachable. Three places outside took that door: two for a return
type they could not avoid naming, and `debug status`, which printed git's raw
`XY` codes.

**The measured surface was much wider than the use.** Of nine modules under
`git/`, six were `pub` and nothing outside touched any of them. Of the eleven
methods, `files()`, `with_before()` had no callers at all, and five of the
remainder existed only for `debug`.

**What a review actually needs is four things.** Open a repository, ask what
changed, ask how much, read one side of one file. That is `Repository`, and
`git` is private behind it.

### Two layers, and the test for each

**A file in `git/` runs one command and parses what it printed into git's own
words.** It decides nothing. So it is named as git spells the command:

```text
run · rev_parse · status · diff/{name_status,numstat} · merge_base · cat_file · worktree
```

`name_status` and `numstat` used to be top-level, named after *flags* while
`cat_file` and `rev_parse` were named after commands — so `numstat.rs` did not
say which command it was a flag of. They are `diff/` now, and the path reads
`git diff --numstat`. Their shared arguments moved to `diff/mod.rs`, where the
forced `--find-renames` is one constant rather than three literals: one saying
a file is a rename while the other counted it as a whole new file would put a
`+400` beside a move.

**A file in `repository/` turns those into the standard format**, and is named
for what a reviewer would call it:

```text
mod.rs          Repository — open, changes, counts, read
diff_type.rs    DiffType — the five ways to compare
changes.rs      Changes — files that share a comparison
changed_file.rs git's records, in the reviewer's terms
```

`worktree.rs` is the one file in `git/` that is not a command — it is
`std::fs`. It stays, because the working tree is one of the three things git
compares, and its own doc says what it is.

### What moved, and why each move was forced

**`Plan` came down.** Turning `DiffType::Staged(rev)` into `["--cached", rev]`
is git knowledge, and it lived in `pipeline/list/resolver.rs` — so a crate two
levels above the backend held a `Vec<String>` of git's flags. It is
`git/mod.rs` now, the door to the directory, which is the one piece of git
knowledge that is not itself a command.

**The list pipeline collapsed from two stages to one.** With planning gone,
what was left was a translation: the repository answers in its own words and
the explorer needs them in its. `resolver.rs` is deleted.

**The request came out of `explorer`.** `ExplorerDiffRequest` was declared
there and **never used there** — measured: zero references outside its own
file. A request is what *produces* the files that crate is handed. It is
`pipeline::list::Request`, and `ExplorerDiffType` became `vcs::DiffType`, named
after the crate that acts on it.

Two `DiffType`s now exist, and they do not collide: `file_types::DiffType` is
how a file is *read* — two columns, one column, alone — and `vcs::DiffType` is
what is being compared. The crate in front of the name says which.

### `debug status` stopped printing what it had no business knowing

It took `vcs::git::Entry` and called `to_file_diff` itself, to print `XY`
against a hand-written manifest — S5's acceptance check. An agnostic layer
cannot print `XY`, so the check moved to where the letters live: `git/status.rs`,
beside the parser that produces them, as a `#[cfg(test)]` module. Sixteen
parser tests moved inline with it from `tests/git_status.rs`, which is where
they belonged anyway — they parse bytes and need no repository.

`Code::letter` is `#[cfg(test)]` now. It is the inverse of the parse, and
nothing draws an `XY` code because nothing outside the crate can see one.

`debug status` prints our model. `debug show` needed a way to name a version no
comparison mentions, which is `Repository::at` — S6's byte-for-byte check
against `git show`, verified still passing through the new layer.

### Verification

611 tests pass, up from 610, with the same five known failures.

Sabotage: forcing `--no-renames` fails the test written for exactly that
(`a_rename_is_counted_the_same_whatever_the_reader_has_configured`); mapping an
unmerged record to `Modified` fails two, one of them a parser test that moved
inline. Mapping a renamed record to `Modified` fails **nothing** — and that is
correct: the paths already say it moved, and `to_file_diff` deliberately reads
`Added` and `Moved` back off them so there is exactly one source. The rename
test now asserts the change type as well as the paths, since it had checked
only half of what it names.

**Cargo did not rebuild** when an edit landed in the same second as the test
command, so the first sabotage falsely passed. Edits and tests as separate
commands is not enough — check for `Compiling` in the output.

## D68 — a feature must not add a file to `render`

**Extended by [D69](#d69)**, which keeps the rule and corrects where the file
list's own pieces sit.

[D65](#d65) split what was `render/explorer.rs` into `render/list.rs` (facts plus
a theme, in text and colour) and `render/fit.rs` (drop the cheap pieces, cut the
longest). It asked the right question of each — *is this general?* — and got
yes, twice. The drop-then-cut rule really is general; the status line needs
exactly it.

It asked the wrong question. The right one is **did this arrive with a
feature?**, and git answers it without argument:

| commit | files added to `render/` |
|---|---|
| `290175c` draw the diff in a terminal | `cells.rs`, `gutter.rs` |
| `cba7e18` one file renders one thing | `column.rs`, `layout.rs` |
| `7f1d035` read a diff inline | — |
| `64fa04e` D65, the file list | **`list.rs`, `fit.rs`** |

Every other brick arrived with the terminal itself. Those two arrived with the
explorer, and `render/list.rs` opened `use explorer::{Content, Guides, Row}` and
matched on `Heading | Directory | File`. `render/mod.rs` says a brick "can be
handed a rectangle and some text by anything", and `lint-arch` enforced that by
banning `crate::view` — the file list's vocabulary walked in through a *crate*
import instead, so the rule never fired. `list.rs` was the file list wearing a
brick's name.

**A tree is not a row.** The thing that kept `render/tree.rs` from being written
as a reusable component is that two unrelated ideas were in one box. Nesting —
who contains whom, what is open, what is therefore visible — has no width and no
colour. A row — content from the left edge, content pinned to the right, and a
rule for what goes when it will not fit — has no idea anything nests. The tree
contributes a *prefix* to a row and nothing else.

Which of the two generalises is settled by naming the second caller, not by
inspection. The row has one: the status line drops a directory, then a rename,
to keep the file name. The nesting does not — a commit graph is a DAG drawn as
**lanes**, `│ ├ ─ ╮ ╰` weaving sideways, with no depth, no ancestors and nothing
to collapse. So the row is what a second view would reuse, and the tree is a
model.

**The shape**, as this decision left it — [D69](#d69) then found the heading
was in the wrong half and moved it, so the files below are not what is there
now:

```text
view/buffer/explorer/     ←→   draw/buffer/explorer/
├ mod.rs   the state           ├ mod.rs   which rows are on screen
├ tree.rs  nests, folds,       ├ tree.rs  guides and fold arrows
│          flattens            ├ list.rs  the flat view — no indent
├ order.rs the two sorts       └ node.rs  one line: text, colour, placing
└ filter.rs the glob
```

The flattening is recorded in the tree rather than walked at drawing time,
because the viewport needs the row count before a frame to clamp the cursor —
a walk that only ran while drawing would be a second answer to how many rows
there are. It records **only which nodes**: `rows: Vec<NodeId>`.

**A visible line is a node, not a `Row` beside one.** The first version of this
kept both, the `Row` carrying the indent — `ancestors: Vec<bool>`, `is_last`,
`is_heading` — on the inherited grounds that a guide is *"a fact about the walk
and not a property of the node"*. That sentence is false, and it survived
because it was moved rather than reread. Folding changes which nodes are
*shown*; it never changes which children a node has. So once `sort` has run,
both "am I the last of my siblings" and "was my ancestor at depth *d*" are
permanent, and `is_heading` was never anything but `node_type == Heading`
written twice.

They live on the node now — `parent: Option<NodeId>` and `is_last: bool`,
recorded once by `Tree::place` — and `draw` walks up the parents to build the
indent. That is a handful of steps for the rows that fit on a screen, against a
`Vec<bool>` allocated per row on every fold, and three fields that could
contradict the tree they described.

The `explorer` crate is gone, and with it `Entry`, `Group`, `Groups`, `Content`
and `vcs::Changes::name`. A file already carries the two revisions it compares,
so which group it is in is a field on it, and what the heading says is
`Revs::heading()` — derived, so nothing can disagree with it. That is
[D57](#d57) finally applied all the way down: the backend used to report
`"Staged Changes"` *and* the revision pair that means it.

**What is *not* shared.** `draw/buffer/explorer/node.rs` places one row and
narrows it, and a commit list would write its own. What both use is
`line_index`, which counts columns for everyone — and miscounting is what
[B9](06-known-bugs.md) actually is: `draw/status.rs:110` says
`path.file_name().chars().count()`. Characters, where a terminal counts columns.
Sharing `fit` was never the fix for that; asking `line_index` is.

### Verification

612 tests pass, up from 610, with the same five pre-existing failures — three in
`codediff/tests/terminal.rs` and two in `codediff/tests/pipeline.rs`, all
confirmed failing on the parent commit in a clean worktree.

Sabotage, counting tests that fail:

| break | tests failed |
|---|---|
| `Rev::Index` heads "Changes" instead of "Staged Changes" | 19 |
| every file put in one group instead of grouped by revision pair | 10 |
| the gap takes one column instead of every spare one | 11 |
| a guide drawn where an ancestor was the last of its siblings | 4 |
| `is_last` recorded as `false` for every node | 17 |
| `parent` never recorded, so the indent has no chain to walk | 13 |

The guide row failed **one** test at first. The shared fixture has no directory
that is both last among its siblings and has children, so every guide column in
it is a `│` and a renderer that drew `│ ` at every depth passed everything.
`an_ancestor_that_was_last_leaves_blank_space_and_not_a_guide` is that tree, on
a real screen. D65 claimed this case failed three tests; it did not.

## D69 — a heading is what an arrangement sits under, not part of one

[D68](#d68) moved the file list out of `render` and into `view` and `draw`. It
left one thing wrong, and the wrongness was visible as dead code: there was a
`draw/buffer/explorer/list.rs` whose whole body asked whether a node was a
directory, in a mode that has none. It could not fire. Nothing noticed, because
there was no screen test of list mode at all — and `explorer_tree.rs` still
asserted `├ ` before a flat path, because its helper built the indent itself
instead of asking the renderer. The test and the screen disagreed and both
passed.

**The cause was that both arrangements owned the heading.** `Tree` made it a
root node with everything hanging off it; the flat mode made it a `Line::
Heading`. So each knew what a heading was, each counted files and summed stats
for it, each held its fold — and each was a *tree* either way, since the flat
one built an arena in order to walk it straight back into one line per file.

A heading is not part of an arrangement. It is what an arrangement sits under.

```text
Explorer
├ "Changes"         ── Style: a Tree, or a List, of that group's files
└ "Staged Changes"  ── the same, arranged the same way
```

```text
view/buffer/explorer/            draw/buffer/explorer/
├ mod.rs     the state           ├ mod.rs        which lines are on screen
├ group.rs   which comparison    ├ tree.rs       guides and fold arrows
├ style.rs   asking whichever    └ view_line.rs  one line: text and placing
├ tree.rs    the nested one
├ list.rs    the flat one
├ order.rs   what comes first
└ filter.rs  the glob
```

Two functions in `group.rs` carry the numbering, because the groups are drawn
one after another and the screen counts every line from zero while a style
counts only its own: `get_heading_line` says which group's heading a line is,
and `get_line_style` translates a screen line into a style and a line within
it. Exactly one of them answers for any line — which was an enum until it was
clear that three of its four callers wanted the same arm every time.

`Explorer` owns the groups, the headings, the counts, the fold on a heading,
and the numbering of lines across groups. A `Style` is handed one group's files
and produces lines; it cannot name a heading because it is never given one.
That is what makes a third arrangement a new variant and nothing else.

An enum rather than a trait, for the reason `BufferType` is one: the set is
closed, so adding a variant breaks the build until it is handled everywhere.

**The order is VS Code's; the method is not.** `SCMTreeSorter` sorts the flat
mode with `comparePaths`, whose one surprising rule is that a shallower file
comes first — `a/z.rs` before `a/b/c.rs` — because the walk runs out of
segments on one side and returns there. Ours did the opposite, since it
compared whole paths as strings and `/` is below every letter. Names now
compare numerically too, so `file9` precedes `file10`.

VS Code folds case *inside* the comparison. Sorting twenty thousand paths makes
about 287,000 comparisons, so that is 570,000 foldings of a forty-character
string; transliterated to Rust it measured **81 ms**, against **25 ms** for the
comparator it replaces. So `order.rs` builds a sort key once per path — twenty
thousand foldings — and the sort is then a memcmp: **1.2 ms**. A key is
deliberately *not* a total order, since two spellings of one name fold
together; whatever sorts carries the path beside it as the tie-break, which is
what a collator's own fallback does.

**A measurement that corrected a claim.** The flat mode was thought to be
avoiding the cost of building a tree. It was not: building the two shapes takes
about the same time, and both are noise beside the 296 ms git spends before
either runs ([D63](#d63)). What cost 19 ms of a 30 ms flat build was the
comparator, in *both* modes. The split is worth making because the flat
arrangement has no directories, no parents and nothing foldable — not because
it is faster.

**A node is what every node has, then what only its kind has.** `NodeType` was
`Heading | Directory | File`. Once the heading left, the honest shape was the
one gitui's `FileTreeItem` and broot's `TreeLine` — the two closest programs to
this — already use: shared fields in the struct, the differing half in an enum
beside them.

```rust,ignore
struct Node { name, parent, is_last, node_type: NodeType }
enum NodeType {
    File { index: usize },
    Folder { children: Vec<NodeId>, open: bool },
}
```

`name` and `parent` are asked of every node while drawing, so they are read
without asking which kind it is. A folder's children and a file's index are
each unreachable from the other — not because a constructor is careful, but
because there is nowhere to put them. An intermediate version used
`file: Option<usize>` with the children beside it, and that was strictly worse:
it made "a file with children" writable, and it took two constructors and 22
tests to forbid what the type above cannot express.

It also caught a real fault immediately. `sort` and `place` descended into
every node writing through a "give me your children" call that a file now has
no answer to, and four tests panicked. The `Option` shape had accepted it in
silence.

And `Content` is now `ViewLine`, the counterpart of [`align::ViewLine`] — one
line of a buffer as facts, before anything draws it. Two crates naming one idea
alike is not the collision [D28](#d28) removed; that was one idea with two
names. `draw/…/node.rs` became `view_line.rs` for the same reason: it draws
headings too, and a heading has no node behind it.

[`align::ViewLine`]: ../../crates/align/src/view_line.rs

**Also gone: `flatten`.** A `bool` on the explorer with no key, no config and
no caller, always `true`. Collapsing a chain of single-child directories is
what `Tree` does; a switch nobody can reach is not a setting.

### Verification

627 tests pass, up from 612, with the same five pre-existing failures — three in
`codediff/tests/terminal.rs` and two in `codediff/tests/pipeline.rs`, all
confirmed failing on the parent commit in a clean worktree.

Sabotage, counting tests that fail:

| break | tests failed |
|---|---|
| a shallower path sorts last instead of first | 8 |
| a run of digits compares as text | 2 |
| a heading's fold does nothing | 3 |
| a heading occupies no line | 4 |

The last one hung, before there was a test for it. Dropping the heading's own
line sends every lookup to a heading, and the scan in `first_file` then never
advances — so the failure was a test run that did not terminate rather than an
assertion. `every_line_resolves_to_a_different_place` names it instead: every
line maps to its own place, and there is exactly one heading line per group. It
fails in both arrangements.

## D70 — an unresolved merge has no index, and a rename needs both its paths

Two failures that had outlived several commits, and both were real rather than
stale assertions.

**A conflicted file was read from a stage git does not have.** Every unstaged
file compared `Rev::Index` against the working tree, which git spells `:0`. A
path in an unresolved merge has stages 1, 2 and 3 and *nothing* at stage 0, so
the read did not come back empty — it failed:

```text
fatal: path 'conflict.txt' is in the index, but not at stage 0
hint: Did you mean ':1:conflict.txt'?
```

`Rev::Conflict(Stage)` had existed since the type was written and nothing ever
produced one. The unstaged side of a conflict is stage 2 — what the reader is
merging *into*, the version they had before the merge began, and so the one the
conflict markers in their working tree are a change to. And a conflict is no
longer listed as staged at all: there is nothing at stage 0 to have staged, and
nothing to review until it is resolved.

**A rename is only visible when both its paths are.** `debug diff-file <path>`
narrowed the status with a pathspec, which is the cheap way to ask git about
one file — but git detects a rename by *pairing* a deletion with an addition,
so a pathspec naming only the new path hides the deletion and `R100
renamed-to.txt` comes back as `A. renamed-to.txt`. The file was found, and
reported as added, with no before side.

So the path is matched here rather than by git: the list is read whole and the
file found by either of its names, which is also what makes the old name work.
That is D58's rule seen from the other side — the list *is* the search, and
narrowing it before searching it throws away what the search needed.

### Verification

633 tests pass and **none fail**, where five had failed on every commit for the
length of this session. The two above were the real ones; the three in
`terminal.rs` were [B8](06-known-bugs.md), which had diagnosed itself and was
waiting for someone to read it.

Sabotage: restoring `Rev::Index` for a conflicted file fails
`every_changed_file_can_be_diffed_without_failing` with git's own error;
restoring the pathspec fails `a_moved_file_is_found_by_either_of_its_paths` on
both of the file's names.

**Two tests were wrong in a way worth recording.** One sliced the status line
at byte 8 to find the path, from when the line carried two status letters
rather than one — so it read `conflict.txt` as `flict.txt` and reported a file
that does not exist. The other asserted `1/4` in a status line that had been
redrawn a digit at a time. Both were asserting on a *representation* rather
than on what is on screen, and both went wrong the moment the representation
moved.
