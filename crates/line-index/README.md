# line-index

Coordinates one UTF-8 line in the forms required by the diff engine and the terminal.

| type | counts |
|---|---|
| `ByteOff` | UTF-8 bytes used for Rust slicing |
| `CharIdx` | Unicode scalar values |
| `Utf16Col` | UTF-16 code units used by the VS Code-compatible engine |
| `CellCol` | terminal columns |

All coordinates are zero-based inside this crate. The engine uses one-based UTF-16 columns; `Utf16Col::from_engine` and `Utf16Col::to_engine` are the only boundary conversions.

For `a日🎉b`, the final `b` begins at byte 8, Unicode scalar index 3, UTF-16 unit 4, and terminal cell 5. A wide character takes two cells, a combining mark takes none, and a tab advances to the next tab stop based on the cells already occupied.

## Ranges

The engine's character mappings are UTF-16 half-open ranges. A range can begin or end inside a surrogate pair, so converting both endpoints by rounding down can erase a real change. `utf16_range_to_bytes` rounds the start down and the exclusive end up, covering the complete character while preserving an empty range as empty.

```rust
use line_index::{LineIndex, Utf16Col};

let line = LineIndex::new("😀", 4);
let bytes = line.utf16_range_to_bytes(
    Utf16Col::from_engine(1)..Utf16Col::from_engine(2),
);
assert_eq!(bytes.start.get()..bytes.end.get(), 0..4);
```

`LineIndex` is for positional queries. `line_index::graphemes` is the allocation-free iterator used when drawing a line. `sanitize` replaces terminal control and bidi formatting characters before text reaches the terminal.

```sh
codediff debug line crates/line-index/fixtures/nasty.txt
codediff debug line crates/line-index/fixtures/nasty.txt --verbose
```

The crate is pure and performs no file I/O; the debug command belongs to `codediff`.
