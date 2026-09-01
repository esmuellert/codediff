# align

Pairs up the two files, line by line, and says where the gaps go.

For `one/two/three/four/five` → `one/three/four/NEW/five`:

```text
view line │ original │ modified │ type
    0     │ line 1   │ line 1   │ unchanged
    1     │ line 2   │ filler   │ deleted
    2     │ line 3   │ line 2   │ unchanged
    3     │ line 4   │ line 3   │ unchanged
    4     │ filler   │ line 4   │ inserted
    5     │ line 5   │ line 5   │ unchanged
```

It returns **line numbers, not text**. View line 1 says "original line 2"; the caller reads
`"two"` from the file it already has. A *file line* is content; a *view line* is a position
it can appear at, and the two counts differ — one file line becomes two view lines inline.

## What it stores

Nothing. It borrows the `LinesDiff` and the two files and computes every answer when asked.

The information was already in the diff — a change of `original 2..3, modified 2..2` says
"one original line, no modified line", which *is* the filler. Storing a view line per line would
mean a 10,000-entry structure for a 10,000-line file, rebuilt on every save, that can
disagree with the diff it came from. It grows with edits, not with file size: the
`comprehensive_move` fixture is 404 lines and 7 changes.

This is VSCode's design. Its `DiffState` is a thin wrapper over the engine result, and its
alignment entries are line-range pairs. Before answering queries, `Alignment` applies
VSCode's `normalizeRangeMapping`, which turns paired ranges reaching both line ends into
line-break ranges. Ours drops the two pixel fields it carries for line wrapping and
plugin-inserted boxes, neither of which a terminal has.

## What it answers

| question | method |
|---|---|
| what is on view line *n* | `view_lines(space)` |
| how many view lines | `view_line_count(space)` |
| which view lines changed | `blocks(space)` |
| which file line is view line *n* | `line_at(space, view_line)` |
| which view line is this file line on | `view_line_at(space, version, line)` |
| which characters changed on this line | `spans(version, line)` |
| which VS Code backgrounds, ranges and empty markers apply | `decorations(version, line)` |
| which hunk is this line in | `hunk_at(version, line)` |
| did this line move | `moved(version, line)` |
| what can be collapsed | `unchanged()` |
| what the engine reported | `changes()`, `moves()`, `hit_timeout()` |

That last row used to be a single `diff()` getter handing out the borrowed engine result,
which read like a verb — "alignment computes a diff" — and left seven `alignment.diff().field`
reach-throughs. VSCode has no equivalent because `DiffState.fromDiffResult` *unpacks* those
values and drops the result, leaving nothing to reach into. We borrow rather than copy, but
the surface is now the same.

## Three things worth knowing

**Two ways to read a diff, one vocabulary.** `DiffType::SideBySide` gives a change as many
view lines as its taller side, pairing the two versions across each view line;
`DiffType::Inline` gives it as many as both sides together, one version per view line. That
single arithmetic difference is the whole of it — both yield the same `ViewLine`, so
fillers, change types, inner-change spans and everything downstream work in either without
a branch.

Neither is a *layout*, which is what this used to be called: a layout is how panes are
arranged on a screen, and which of these to walk is settled long before anything knows how
wide a pane is. The type lives in `file-types` as `DiffType`, beside the third way of
reading a file — `Single`, which has nothing to pair against. See
[D60](../../docs/plan/05-decisions.md#d60).

View line 40 in one is not view line 40 in the other, which is why switching translates
through `line_at` and `view_line_at` rather than keeping the number.

**`Original` and `Modified`, never left and right.** Left and right are places on a screen.
Inline view draws both on one side, so a model naming them cannot describe it.

**A move is not a type of view line.** The engine reports a moved block as an ordinary deletion
plus an ordinary insertion, and its move ranges need not agree with its change ranges — in
`comprehensive_move` a move covers original 32..89 while a change covers 37..139. So moves
are a lookup by line number. VSCode has `movedTo`/`movedFrom` fields on `DiffMapping`
commented out, having reached the same place.

**A hunk's identity is its content, not its position.** `HunkId` hashes the hunk's text, so
inserting a function above a reviewed hunk does not mark it unread, while editing it does.

## What it deliberately does not know

Pane width. Once wrapping is on, a line is no longer one view line and pairing depends on width, so
the wrap-aware alignment lives in `ui` — the same split VSCode makes, with `DiffState`
width-independent and `computeRangeAlignment` in its view. See D19.

## Checking it

```sh
codediff debug align vendor/test-pairs/block_moved_down/{original,modified}.txt
```

The left column must read as exactly the original file and the right as exactly the
modified one — which is the property the tests assert over all twelve fixtures.
