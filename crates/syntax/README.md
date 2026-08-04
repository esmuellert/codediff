# syntax

Says what a piece of text *is* — a keyword, a string, a comment — and how much of it, so
`ui` can colour it.

Real highlighting arrives at **S11**. Until then `Plain` returns nothing, which is not a
placeholder to be deleted: highlighting is too slow to run on the render path, so a renderer
must already cope with having no spans yet. This is that state, made explicit and testable.

## Where the design comes from

Everything below was taken from three tools that have already solved this, rather than
invented.

| tool | why it is a reference |
|---|---|
| [VS Code](https://github.com/microsoft/vscode) + [vscode-textmate](https://github.com/microsoft/vscode-textmate) | the same grammar family we use, and the only one of the three with a diff editor |
| [`delta`](https://github.com/dandavison/delta) | Rust, terminal, read-only, **a diff viewer with syntect** — the closest tool that exists |
| [`bat`](https://github.com/sharkdp/bat) | Rust, terminal, syntect; the production-hardened input layer |

**The governing rule for what we take:** a feature is included unless it exists *only* to
support editing. Nothing is dropped for being inconvenient. Where something is left out, the
reason is written down below. The two research passes behind this are recorded as
[D36](../../docs/plan/05-decisions.md).

## The one thing we get for free that `delta` cannot

`delta` reads a unified diff from stdin, so it only ever sees hunk fragments. TextMate
parsing is stateful, so it resets the highlighter at every hunk header
([`handlers/hunk_header.rs`](https://github.com/dandavison/delta/blob/main/src/handlers/hunk_header.rs)),
and a hunk starting inside a block comment is highlighted as code — or worse, the code after
it is highlighted as a comment. This has been open since 2020:
[#117](https://github.com/dandavison/delta/issues/117),
[#162](https://github.com/dandavison/delta/issues/162), and five more. The proposed fix is to
ask git for `-U9999` and throw most of the result away.

**We read whole file snapshots from git**, so we parse from line 1 and the entire class of bug
does not exist. This is the largest advantage our input format gives us, and it costs nothing.

## How the two colour layers compose

The rule all three tools converge on, and the reason syntax survives inside a diff:

```text
diff   →  background, and every other attribute
syntax →  foreground only, and only where the diff style opts in
```

VS Code renders `<span class="mtk5 char-insert">` — `.mtk5` sets `color`, `.char-insert` sets
`background-color`, and its diff stylesheet never sets a foreground. `delta` does the same in
`superimpose_style_sections`, with an explicit `is_syntax_highlighted` bit per style, and
treats *"`syntax` used as a background colour"* as a fatal configuration error.

Two consequences:

- **Character-level change highlighting is a different *background*, not a different
  foreground.** Otherwise it fights the syntax colour instead of layering under it.
- **Merging two independent segmentations of one line needs an assertion.** `delta` explodes
  both runs to one style per character, zips them, and **panics** if the two character streams
  disagree. The silent failure is colours shifted by a few columns, which is very hard to find
  later.

## What we take, and what we do not

### Taken

| | why |
|---|---|
| Whole-file parse from line 1, per version | correctness `delta` cannot reach |
| Viewport first, then fill in during idle | VS Code's shape; `poll()` gives it to us with no thread |
| **Full scope-selector themes**, not abstract categories | see below |
| Font style as well as colour — italic, bold, underline, strikethrough | four independent bits, and themes set them without a foreground |
| Size and line-length limits | VS Code: 20 MB / 300 K lines disables it, 20 000 characters skips a line |
| Long lines cut off `bat`'s way, not `delta`'s | `bat` swaps in `"\n"` and preserves the parse state; `delta` truncates and corrupts it |
| Tab stops computed **before** highlighting | otherwise spans and screen columns disagree. `delta`'s expansion is column-blind, so `a\tb` and `aaaaaaa\tb` do not align |
| Invisible and confusable character detection | VS Code's `unicodeTextModelHighlighter`. For a tool built to review generated code this is a feature, not a nicety |
| Bidi and zero-width sanitisation, `from_utf8_lossy`, BOM stripping, UTF-16, binary detection | `bat`'s preprocessor. Trojan Source is a real attack on a diff viewer |
| 24-bit → 256 → 16 colour downgrade | without it the `ansi` and `base16` themes render as garbage |
| First-line and shebang detection | we hold the file; `delta` usually cannot |

### Left out, with the reason

| | why not |
|---|---|
| Invalidation queue, convergence early-out | exist to re-tokenise after a keystroke |
| Per-line parse-state cache | exists to resume from an edit at an arbitrary line. We only ever extend forward and keep every span (~600 KB for 5 000 lines), so one state at the end of the parsed prefix is enough |
| VS Code's **guessed** start state | exists because parsing the prefix exactly would freeze their UI. Ours is four times faster, and the idle pass closes the window. Guessing would make us wrong where we are exact |
| Web worker | JavaScript cannot block its UI thread; `poll()` is our equivalent |
| Semantic tokens | need a language server. Note that the `invalid` scope gives error colouring without one |

### Not left out — simply not this milestone

Bracket-pair colourisation, indent guides, whitespace rendering, sticky scroll. None is
edit-specific, so none is excluded on principle; they are separate features rather than part
of syntax highlighting.

## Why the token list is *not* short

An earlier draft of this crate normalised everything to ten abstract kinds so that `ui` owned
all colour. Measurement killed it. VS Code's `dark_plus` resolves **65 rules over ~190 scope
selectors**, and needs roughly **20–25 distinct colours** to look like itself:

- `keyword` and `keyword.control` are different colours; so are `string` and `string.regexp`
- `meta.template.expression` **resets to the code colour**, so interpolated code inside a
  string is coloured as code rather than as string
- rules are language-qualified (`storage.type.java`, `keyword.operator.logical.python`) and
  parent-scope-contextual (`"source.css entity.other.attribute-name.class"`), which a flat map
  from ten categories structurally cannot express

`delta` and `bat` both use `.tmTheme` files at full fidelity for the same reason. Ten buckets
would make our Catppuccin an impression of Catppuccin rather than Catppuccin.

What survives from the original goal is the *shape*: this crate still says **what text is**,
not what colour it should be. It reports the scope that matched, and `ui` owns the mapping —
the difference is that a scope is a path, not one of ten names.

**As built:** 31 roles over 78 scope selectors, from Catppuccin's own
`groups/{syntax,treesitter}.lua`. Every one of the 78 is proved to claim something real in
`crates/ui/tests/scopes.rs`, against a corpus of seventeen languages — the engine accepts a
selector it can never match, so a typo costs a colour and says nothing otherwise.

## What a span carries: a pen, not a colour

A `Span` says "bytes 4..9 are pen 12". It never names a colour, and this crate never learns
what pen 12 looks like. `ui` hands in the rules and owns the table.

Three things fall out, and each is a reason:

- **A terminal with no 24-bit colour can be highlighted.** `basic-dark` exists precisely
  because Catppuccin's diff backgrounds collapse when a terminal quantises them; handing that
  theme RGB syntax colours would break its one invariant. With a pen it answers
  `Color::Indexed` instead, and a test asserts it emits no 24-bit sequence anywhere.
- **Changing theme costs nothing.** No span mentions a colour, so nothing is re-read.
- **The scope table is one shared constant**, not one per theme, because which scopes are
  keywords is a fact about TextMate rather than taste.

VS Code does exactly this: its token metadata packs an index into a `ColorMap`.

## Measured

Release build, this palette, real Rust source:

| | |
|---|---|
| grammars unpacked (once) | 1.3 ms |
| scope table compiled (once) | 60 µs |
| first frame, 24 lines | 3.1 ms |
| throughput | ~18 500 lines/sec |

18 500 rather than the 45 000 an earlier two-rule bench showed: a real theme means every
scope change is matched against 78 selectors, which is what `bat` pays too. Two consequences
are in the code rather than in a comment:

- the engine's matcher is built **once per batch**, not once per line — that alone was a
  third of the total, and is why `Engine::read` takes a slice
- a frame colours at most `limits::LEAP` lines and draws the rest plainly, so `G` in a
  300 000-line file cannot freeze the interface for sixteen seconds. The idle pass finishes
  the job. That is VS Code's time-slicing, budgeted in lines because `ui` may not have a
  clock.

## Layout

```text
crates/syntax/
├── src/
│   ├── lib.rs          what a caller sees
│   ├── style.rs        Pen, Style, Rule, Span, and coalescing
│   ├── detect.rs       which language: name, then extension, then shebang
│   ├── highlighted.rs  one file, coloured as far as anyone has looked
│   ├── limits.rs       every threshold, in one place
│   └── engine/
│       ├── mod.rs      the seam: everything above is engine-free
│       └── syntect.rs  the only file allowed to name syntect
└── tests/
    ├── languages.rs    fourteen languages, by the pen that reaches the caller
    ├── multiline.rs    block comments and multi-line strings — delta's bug, as a test
    └── nasty.rs        tabs, bidi, invisible characters, 20 000-character lines
```

The other half is in `ui`, because it is about colour:

```text
crates/ui/
├── src/
│   ├── highlight.rs        the join: the process-wide engine and scope table
│   └── theme/
│       ├── code.rs         Token — what a piece of code is — and each theme's colours
│       └── scopes.rs       which scope path wears which pen
└── tests/
    ├── scopes.rs           every selector claims something; every token is worn
    ├── colours.rs          the right one wins, word by word, in eleven languages
    └── corpus/             the source those two are run against
```

`cargo xtask lint-arch` refuses the name of a syntax engine anywhere outside
`crates/syntax/src/engine`, and refuses IO anywhere in this crate.

There is no `scope.rs` or `theme.rs` here as the plan sketched: matching scope selectors with
TextMate precedence is subtle, well-tested work that the engine already does, and rewriting it
to avoid a dependency we have already taken would have no reader-visible result. There is no
`text.rs` or `unusual.rs` either — `line-index` already answers tabs, widths, control
characters and bidi, and a `Span` is a byte range, so unlike `delta` we never have to expand
tabs before highlighting.

## What it deliberately does not know

Which file it is looking at, and what a diff is. A highlighter is given a language and some
lines. That `ui` paints the result under a diff background is not its business — which is why
the composition rule above is implemented in `ui`, not here.
