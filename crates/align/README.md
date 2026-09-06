# align

Pairs the original and modified text described by a `diff-types::LinesDiff`.

`Alignment` owns the engine result and shared copies of both line arrays. It does not store a row for every line. Instead, it derives view lines, fillers, change blocks, hunks, character decorations, and move lookups from the result when asked.

The model uses `DiffVersion::Original` and `DiffVersion::Modified`, not screen-specific left and right names. `file-types::DiffType` selects a view projection:

- `SideBySide` pairs the two versions across one row sequence;
- `Inline` emits the versions in one sequence with two optional gutters;
- `Single` is handled by the pipeline and has no alignment.

Side-by-side alignment uses the engine's inner mappings to split some change blocks before placing fillers. Character ranges are converted from the engine's UTF-16 coordinates to byte and terminal-cell coordinates through `line-index`.

The crate performs no I/O. Its normal dependencies are `diff-types`, `file-types`, and `line-index`; the engine is used only by tests as an oracle.

```sh
codediff debug align original.txt modified.txt
```
