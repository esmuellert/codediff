# ui

Draws the review interface, and owns the terminal while it runs.

It is handed the text of one or two files and paints them. It cannot see a repository,
compute a diff, or read a file — `cargo xtask lint-arch` fails if it tries.

## The shape

Four levels, each containing the next, borrowed from Neovim because the problem is the same
one: several things on screen, each with its own contents, position and keys.

```text
View     tabs, and every buffer any of them can show
└ Tab    a layout of panes, and which has focus
  └ Pane one buffer, and one Viewport onto it
    └ Viewport   top, cursor, left
```

Two rules follow from it, and between them they decide where almost everything lives.

**Containment order is execution order.** An action is carried out by the lowest level that
contains everything it affects. A motion affects one viewport, so the focused pane's buffer
does it. Resizing a pane border affects *two* panes, so only the tab can. This is why
`Action` has the arms it has, and no others: an arm exists iff it has an executor no other
arm has.

**Buffers live in `View`, referenced by `BufferId`.** Never by reference — a pane holding
`&mut Buffer` would make the whole structure self-referential. Helix does the same with
`DocumentId`/`ViewId`; Zellij's `Box<dyn Pane>` is the counter-example and forced
`Rc<RefCell<_>>` throughout. Position is on the *pane*, so two panes over one buffer scroll
independently.

See [D27](../../docs/plan/05-decisions.md#d27).

## A buffer is a sequence of view lines you can scroll through

That is the whole definition.

```text
struct Buffer {                  // what every buffer has
    view_lines: u32,
    blocks: Vec<Range<u32>>,     // empty when there is nothing to compare
    exhausted: Option<Direction>,
    buffer_type: BufferType,
}

enum BufferType {                // only what differs
    SideBySide(SideBySide),      //   { diff, divider }
    Inline(Inline),              //   { diff }
    SingleFile(SingleFile),      //   { file, lines }
}
```

A buffer is a *projection*, not the data. `Diff` — what the pipeline delivers — is one
file's two versions and the pairing between them, and says nothing about view lines. How
many there are is not a fact about a diff: it is an answer to *how would this be laid out*.
`Buffer` holds that answer, computed in one place from the kind that decided it, so the two
cannot describe different layouts.

Rust has no inheritance, so a shared base is composition plus an enum naming the
alternatives. Everything true of any buffer — the row count, the changed blocks, stepping
between them, the note when a change key has nowhere left to go — is written **once**, on
`Buffer`. `SideBySide` adds one field. `Inline` adds nothing, which is the finding rather
than an oversight: reading a diff inline needs no state that reading it in columns does not.

Side by side and inline are separate variants rather than one with a flag, so the **variant
is the layout** — there is no field for the row count to fall out of step with, and both
the renderer and the keymap dispatch on it without reading one.

What `Buffer` cannot own is the switch. It changes what a row *is*, so the pane's cursor has
to move at the same moment — and a row number does not survive the trip, since row 40 side
by side is a different line inline. The *line* survives, so the view translates through it.
See D31.

An `enum`, not a trait: the kinds are a closed set, so adding one breaks the build until it
is handled everywhere — and a trait could not carry the shared fields anyway.

A `Diff` holds an `Alignment`, which **owns** both files and the engine's result. The
pipeline builds it once, when the file is opened, and drawing a frame is pure reading.

That is not a small detail. While `Alignment` borrowed, it could not be returned by the
stage that built it — the texts it pointed at died with the call — so the pipeline had to
lend it through a closure, and every type holding one grew a lifetime:
`Diff<'a>` → `Session<'a>` → `View<'a>` → `Tab<'a>` → `Pane<'a>`. Making it own its files
deleted the closure, the lifetimes, and all per-frame work at once. See
[D27](../../docs/plan/05-decisions.md#d27).

## One column or two

`Viewport` holds **one** scroll position and **one** cursor, whatever the buffer draws with
them:

```text
Viewport { top, cursor, left }
           └ shared by every column ┘
```

The two columns of a diff are not separate scrollable things kept in agreement. They are one
position, drawn in the same call from the same slice of view lines. There is nowhere for them to
disagree, so there is no synchronisation code, so there is nothing to get wrong.

The plugin needed 415 lines of `scrollsync.lua` for this and got it wrong twice. VSCode
gives each editor its own `scrollTop` and holds them in a bidirectional constraint with
write guards. See [D19](../../docs/plan/05-decisions.md#d19).

A diff always has **two** columns, so neither field of `Frame` is optional. A file with one
side is not compared against anything, so it is not a diff at all — it is a `SingleFile`
buffer, drawn by `draw/buffer/single_file.rs` in a single column. Both diff modes fall back to
it, because with one version there is nothing to lay out against. Nothing on it changed *relative to* anything,
so nothing is highlighted. Marking every line of a new file green says nothing the word "added" does
not. VSCode arrived here from the same bug and stopped opening a diff editor for added,
untracked and deleted files entirely.

The trigger is *absent*, never *empty*. A tracked file emptied to zero bytes still has a
side to compare against, so it gets two columns and a diff showing every line deleted. See
[D23](../../docs/plan/05-decisions.md#d23).

## What is where

```text
view/                what is on screen, and where
├── mod.rs             View — tabs, and every buffer any of them can show
├── tab.rs             Tab — a layout of panes, and which has focus
├── pane.rs            Pane — one buffer, and one Viewport onto it
├── viewport.rs        Viewport — top, cursor, left
└── buffer/            what a pane can show
    ├── mod.rs           BufferType — the closed set of kinds
    ├── buffer.rs        Buffer — view lines, changed blocks, change navigation
    ├── colour.rs        asking the colouring thread, and reading the answer
    ├── side_by_side.rs  SideBySide — a diff, and its column divider
    ├── inline.rs        Inline — a diff, one version per view line
    ├── single_file.rs   SingleFile — one version, shown alone
    └── explorer.rs      Explorer — the list of changed files
draw/                what each buffer type looks like
├── screen.rs          the screen: body and status line
├── tab.rs             every pane the tab has, and the border between two
├── pane.rs            one buffer, at the height its rectangle gives it
├── buffer/            what a buffer type looks like
│   ├── mod.rs           the one place a BufferType is dispatched on
│   ├── side_by_side.rs  one pane holding a diff in two columns
│   ├── inline.rs        one pane holding a diff one version per view line
│   ├── single_file.rs   one pane holding one version of a file
│   └── explorer.rs      one pane holding the list of changed files
└── status.rs          the bottom row
render/              putting characters and colour on a cell grid
├── layout.rs          where the columns and gutters go
├── column.rs          one column's view lines
├── gutter.rs          one line number
├── line.rs            how one line of a diff is coloured
├── list.rs            what one row of the file list says, and its colour
├── fit.rs             what survives when a row is wider than its pane
└── cells.rs           one line onto one row of cells
syntax/              colouring, on a thread that is not the one drawing
├── mod.rs             Syntax — the worker, and one request in flight per file
├── message.rs         SyntaxRequest / SyntaxResponse — all that crosses
├── store.rs           Store — every colour, keyed by git's name for it,
│                     dropped least recently used
└── worker.rs          the loop, and where it left off in unfinished files
input/               what does this key mean
theme/               what colour is it — taste, and nothing else
├── code.rs            a piece of code, by what the language says it is
├── tree.rs            a part of a tree drawn in rows
├── change.rs          a file, by what happened to it
├── catppuccin.rs      four flavours, by their arithmetic
├── basic.rs           the sixteen colours every terminal has
└── colour.rs          colour arithmetic
start.rs             opening a review, and everything it needs before frame one
app.rs               read a key, dispatch it, draw a frame
terminal.rs          who owns the screen, and how it is given back
```

**The module tree is the model.** Each of the four levels is one file, in containment
order, so `ls view/`, `ls draw/` and the diagram above are the same picture. `buffer/` is
*inside* `view/` because `View` owns the buffers — Neovim's are global and Helix keeps
`documents` beside its `tree`, but both have an editor above that owns the two. We do not,
and inventing one to justify a directory would be the tail wagging the dog.

`draw/` mirrors it level for level: `screen.rs` hands `tab.rs` a body, `tab.rs` hands
`pane.rs` a rectangle, `pane.rs` hands `buffer/` a height. Nothing above is told what type
of buffer is below it, and nothing below is told how many panes there are. `viewport.rs`
has no counterpart because a position draws nothing, and `status.rs` has no counterpart
because it is not a level — it is the row beneath the body.

An id lives with the collection it indexes: `BufferId` in `mod.rs` beside `View::buffers`,
`PaneId` in `tab.rs` beside `Tab::panes`.

`terminal.rs` is separate from everything else because it is the only part that can leave a
shell broken: raw mode and the alternate screen have to be undone on quit, on error, on
panic and on suspend. It is tested through a real pty, from outside the process.

## Keys

A key resolves to a `Command`, and every command is one of exactly three kinds
— split by **who answers it and how long they take**, not by whether it has a side effect,
because that is the question the loop actually has to act on:

| arm | executed by | can fail | latency |
|---|---|---|---|
| `Buffer(BufferAction)` | the focused pane's buffer | no | µs |
| `Pane(PaneAction)` | the focused pane | no | µs |
| `Tab(TabAction)` | the active tab | no | µs |
| `View(ViewAction)` | the view | no | µs |
| `Program(ProgramAction)` | whoever owns the terminal | no | µs |

One arm per executor, and each arm's payload is that executor's own set of commands,
named `<Executor>Action`. The first four are the levels of the view model, innermost
first; `Program` is not a level and sits below all of them.

**Nothing here blocks and nothing leaves the crate.** There used to be a sixth arm,
`Task`, for the one action that needed a repository: it was *returned* rather than run,
because `ui` could not reach git. The pipeline answers on a thread now, so opening a file
costs a `send` and the arm is gone. See [D59](../../docs/plan/05-decisions.md#d59).

`input/resolver.rs` **resolves**; `app.rs` **dispatches**. Keeping those apart is why the
resolver can be a pure function of its own state and one key: no clock, no IO, no view. A
test is a string of keys.

**Each level owns its commands and binds them** — one file per executor, holding the actions
*and* the keys, so a new command is one file rather than two:

```text
buffer.rs    motions, and whatever a buffer kind adds   ← innermost, consulted first
pane.rs      one pane, about its own view of a buffer
tab.rs       a tab, about its panes: focus, resize, zoom
view.rs      the whole view, about its tabs
program.rs   quit, suspend, redraw — below every level, consulted last
```

**Lookup walks that order**, so an inner level *shadows* an outer one. That is what lets the
explorer bind `<` to collapse the sidebar while a diff keeps `<` for its column divider:
the diff's own binding is found first, and in the explorer the chain falls through. Neovim's
buffer-local-over-global, and the reason a key's scope and a key's executor need not be tied
together.

`Context` selects only the innermost list — which *buffer kind* has focus. Every level above
binds the same keys whatever is focused.

### The table is data

Each level's bindings are a `const` list — `crokey`'s `key!()` is const-capable, which is the whole
reason to depend on it. A command is a **value**, never a closure: a closure could not be
printed into a help screen, compared in a test, or held without capturing references to
everything it might touch.

crokey covers **what one key is** — literals, `KeyEvent` conversion, formatting for help,
and config parsing later. Its "combination" means keys pressed *together*; a sequence like
`gg` is keys pressed *one after another*, which is ours.

### No binding is a prefix of another

Lookup gives the flat list trie semantics: **commands live only at leaves**. `gg` existing
means bare `g` is unbound, and `j` being bound means nothing may follow it.

That is what vim's own built-ins do — `g`, `d`, `z`, `[`, `]` are all unbound alone — and
it is why the resolver needs no clock. Ambiguity has no good resolution: firing immediately
makes the longer binding unreachable, waiting makes the shorter one feel broken. Vim needs
`timeoutlen` only because user mappings *may* create ambiguity. A test enforces the rule,
so relaxing it later means adding a clock and deleting one test, not reshaping the
resolver.

Two consequences worth knowing:

- **Escape cancels what is in flight, and only then.** With nothing pending it falls
  through to the table, where it quits. Without this, pressing `g` and changing your mind
  would exit.
- **`0` is a digit mid-count and a motion otherwise** — vim's rule, and the only place
  counts and bindings interact.

## Colours

A `Theme` is a table of `Style`s, one per role, and nothing else. It has no idea what a
hunk is, and the renderer has no idea what Catppuccin is; the two meet at the field names.
Styles compose by `patch`, which overrides only what is set — so a role supplies a
background and inherits the foreground, and priority is the order of the patches.

Six themes, in two families:

| | |
|---|---|
| `catppuccin-mocha` (default), `-macchiato`, `-frappe`, `-latte` | exact 24-bit colours |
| `basic-dark`, `basic-light` | the terminal's own background, and the 256-colour cube |

**Catppuccin is reproduced by its arithmetic, not by a list of hex values.** Its diff
backgrounds are an accent blended into the base at a fixed opacity:

```text
out = round(alpha × accent + (1 − alpha) × base)

line added     18% green      inner change  30% green
line removed   18% red        inner change  30% red
moved block     7% blue       cursor line   64% surface0
```

Those are the opacities `catppuccin/nvim` uses for `DiffAdd`, `DiffDelete`, `DiffChange`
and `DiffText`. So a flavour is 26 palette colours and a shared derivation, and a test
asserts the derivation still reproduces Catppuccin's own published results. A new flavour
is 26 numbers.

**`basic` exists because Catppuccin's subtlety is also its failure mode.** Eighteen percent
of an accent is a few points of lightness; a terminal without 24-bit colour rounds that
straight back into the background, leaving a diff with no visible diff in it. `basic` names
nothing exactly — `Color::Reset` for the background, so it inherits whatever scheme the
reader already runs — and a test asserts it never emits a 24-bit colour at all.

Which one you get, absent `--theme`, is decided by `COLORTERM` and `COLORFGBG`: the only
things already known for free. There is a real way to ask a terminal its background — an
OSC 11 query — but it needs a round trip the terminal may never answer, and a reviewer
waiting on a timeout before the first frame is worse than a wrong guess they can override.
`codediff doctor` prints what was detected and why.

## Three things that are easy to get wrong

**A changed line is coloured to the edge of its column**, not to the end of its text.
Otherwise a short changed line reads as a ragged stripe. Neovim calls this `hl_eol`.

**A grapheme cluster is not a column.** A double-width character straddling either edge of
the viewport, and a tab, are drawn as spaces, because half of a wide character cannot be
drawn and drawing all of it would shift every column after it.

**The file being reviewed must never reach the terminal unaltered.** `ESC` starts a
sequence a terminal *obeys*, and `U+202E` reorders a line so it reads as something other
than what it says. Both are replaced by a stand-in of the same width, in `line-index`,
beside the code that measures them — so the substitution and the measurement cannot drift.

## Component gallery

The compiled binary can open deterministic fixtures around the production components:

```sh
codediff debug ui
codediff debug ui --list
codediff debug ui side-by-side/replacement
```

With no story ID, the binary opens a searchable catalog. `j`/`k` select, `/` filters, and
`Enter` opens a full-screen preview. In a preview, `Esc` returns to the catalog, `[`/`]`
move between stories, `r` resets the fixture, and `q` exits. A direct story ID remains the
shortest reproducible path for a bug report.

The catalog deliberately separates its visual roles: lavender group headings, blue Story IDs,
muted descriptions, and a lavender selected ID over the cursor-line background. It uses no bold
text; colour, background, and the `›` marker carry the hierarchy. Its navigation bar uses two
rows so the current screen and its colour-coded key/action pairs cannot read as one long sentence.

The same catalog drives `Harness` snapshots and PTY tests, so manual inspection and automation
do not maintain separate mock renderers. For a pipeable text frame, use:

```sh
codediff debug ui side-by-side/replacement --snapshot --width 100 --height 24
```

Fixture construction stays in the `codediff` composition root. Typed builders create real
`File` and `DiffContent` values; two-sided fixtures still pass through the production diff and
alignment pipeline. Every SideBySide and SingleFile story receives a real `SyntaxService` and
syntax worker; an empty file has the same service but naturally issues no request. Long-line
fixtures are generated to at least 512 terminal cells, measured with `LineIndex`, so they still
overflow a wide terminal. Small canonical stories isolate one behaviour, while `mixed-status`,
`awkward-paths`, `comprehensive`, `empty`, and `large-syntax` stories combine edge cases. Each story mounts `Explorer`, `SideBySide`, `SingleFile`,
or `DiffViewer` from this crate rather than copying its drawing code.

## Checking it

```sh
codediff <path>
```

`q` quits, `j`/`k` scroll, `]c`/`[c` step through changes, `t` switches between side by
side and inline, `>`/`<` drag the divider, `Ctrl-Z` suspends. The screen must match `codediff debug diff-file <path>` row for row.

```sh
codediff <path> --theme basic-light
```

Every theme must draw exactly the same characters — only the colours may differ.
