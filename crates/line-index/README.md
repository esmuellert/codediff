# line-index

Tells you where each character of a line sits.

For the line `a日🎉b`:

| grapheme | byte | `byte_to_cell` | width |
|---|---|---|---|
| `a` | 0 | column 0 | 1 cell |
| `日` | 1 | column 1 | **2 cells** |
| `🎉` | 4 | column 3 | **2 cells** |
| `b` | 8 | column 5 | 1 cell |

Nothing is one-to-one with anything: `日` is 3 bytes, 1 character, and 2 columns.

## What it is for

The C diff engine reports positions as **UTF-16 columns**, because it mirrors VSCode and
JavaScript strings are UTF-16. Rust slices strings by **byte offset**. A terminal draws at
**cell columns**. Something has to translate, and this is it.

```rust
use line_index::{LineIndex, Utf16Col};

let line = LineIndex::new("let x = \"日本\";", 4);

// The engine says "column 10"; find it in the bytes, then on screen.
let byte = line.utf16_to_byte(Utf16Col::from_engine(10));
let cell = line.byte_to_cell(byte);

assert_eq!(byte.get(), 9);
assert_eq!(cell.get(), 9);
```

It works on **one line at a time** and knows nothing about rows, panes, scrolling or
colour. It supplies the **x** coordinate; `align` supplies the y.

## The four coordinate systems

| type | counts | needed by |
|---|---|---|
| `ByteOff` | UTF-8 bytes | slicing `&str` |
| `CharIdx` | Unicode scalar values | `str::chars` |
| `Utf16Col` | UTF-16 code units | **the diff engine** |
| `CellCol` | terminal columns | **the display** |

They are separate types because on ASCII they are all the same number, so mixing them up
survives every test until someone opens a file containing an emoji.

`Utf16Col` is the only one that meets the engine's one-based convention, and
`Utf16Col::from_engine` is the single place that adjustment happens.

## Two rules worth knowing

**A tab's width depends on where it starts.** Four columns at position 0, one column at
position 3. It is a running total over the line, not a property of the character.

**`cell_to_byte` has no exact answer inside a wide character.** Column 2 is the right half
of `日`; no byte begins there. It returns the character's start, so horizontal scrolling to
that column has to draw a pad rather than a character.

## Converting a range, not a position

The engine's inner-change spans need `utf16_range_to_bytes`, not two calls to
`utf16_to_byte`. It compares **individual UTF-16 code units**, so it can report a span that
begins or ends *inside* a character:

```text
😀 = D83D DE00        🨀 = D83E DE00        they differ only in the high surrogate
engine reports        L1:C1-L1:C2           one unit — half a character
```

Rounding both ends down gives `0..0`, and the change is highlighted nowhere at all.
So the start rounds **down** and the exclusive end rounds **up**:

```rust
use line_index::{LineIndex, Utf16Col};

let line = LineIndex::new("😀", 4);
let bytes = line.utf16_range_to_bytes(Utf16Col::from_engine(1)..Utf16Col::from_engine(2));

assert_eq!(bytes.start.get()..bytes.end.get(), 0..4); // the whole character
```

An empty span stays empty, so "nothing changed here" is still distinguishable. Rounding is
to whole *characters*; a caller wanting whole grapheme clusters, so a combining mark
travels with its base, can widen the result using `graphemes`.

## Measuring versus drawing

`LineIndex::new` allocates an index on any line that is not plain ASCII. That is worth it
for positional queries, and waste for a renderer walking a line to draw it — which it does
for every visible line on every frame. So there are two entry points:

| you want to | use |
|---|---|
| ask where something is | `LineIndex`, built once and kept |
| walk a line to draw it | `line_index::graphemes(text, tab_width)`, no allocation |

## Checking it

```sh
codediff debug line crates/line-index/fixtures/nasty.txt [--verbose]
```

Lists the characters whose byte, UTF-16 and column positions disagree, plus any control
characters. Plain ASCII lines are skipped — there all three are the same number, which is
why confusing them survives every test until a file contains a tab or an emoji.

```text
  line 13  "tab then    日本    then    emoji 🎉"
    ├─ ⇥   byte   3   utf16   3   column   3   width 1
    ├─ ⇥   byte   8   utf16   8   column   8   width 4
    ├─ 日  byte   9   utf16   9   column  12   width 2
    ├─ 本  byte  12   utf16  10   column  14   width 2
    ├─ ⇥   byte  15   utf16  11   column  16   width 4
    ├─ ⇥   byte  20   utf16  16   column  24   width 4
    └─ 🎉  byte  27   utf16  23   column  34   width 2
```

Tabs print as `⇥` so they cannot be mistaken for the literal word "tab" in the text.

`--verbose` lists every character instead, and adds a `^-` map beneath each line — `^`
where a character starts, `-` for the columns it continues into — against a cell ruler:

```text
         mixed 日本 and ASCII on one line
         ^^^^^^^-^-^^^^^^^^^^^^^^^^^^^^^^
         0····+····1····+····2····+····3·
```

A wrong width shifts every marker after it, so the error is visible at the position it
occurs. That checks columns against what the terminal actually draws. It cannot check byte
or UTF-16 offsets, which are invisible on screen — `fixtures/nasty.expected` covers those.
