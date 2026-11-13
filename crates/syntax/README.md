# syntax

Produces syntax spans for complete file snapshots. A span contains a byte range and a `Pen`; the UI maps pens to the active theme. This crate does not choose terminal colours or know about diffs.

The engine selects one implementation per file:

- Tree-sitter for the languages with bundled highlight queries;
- syntect with two-face/TextMate grammars as the coverage fallback.

Language detection uses the file name, extension, and shebang. Colouring starts at the beginning of a file so multiline comments and strings retain their parser state. Tree-sitter may parse the whole file at once; TextMate continues from its saved line state. `Highlighted` hands spans back as far as it has read; the `pipeline` crate's syntax thread calls it and stores the results.

Files that are binary, too large for the configured highlighting limits, or unknown to both engines remain readable as plain text. A missing span means “not coloured yet” or “no rule claimed this text”; it is not a rendering error.

The public seam is `Engine`, `Grammar`, `Palette`, `Highlighted`, `Span`, and `Style`. Engine-specific names stay under `src/engine/`.
