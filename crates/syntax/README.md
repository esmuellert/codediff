# syntax

Produces syntax spans for complete file snapshots. A span contains a byte range and a `Pen`; the UI maps pens to the active theme. This crate does not choose terminal colours or know about diffs.

The engine selects one implementation per file:

- Tree-sitter for the languages with bundled highlight queries;
- syntect with two-face/TextMate grammars as the coverage fallback.

Language detection uses the file name, extension, and shebang. The worker starts at the beginning of a snapshot so multiline comments and strings retain their parser state. Tree-sitter may parse the whole file for a request; TextMate continues from its saved line state. Responses are returned in chunks and the UI-side `Store` keeps spans for files and versions it has requested.

Files that are binary, too large for the configured highlighting limits, or unknown to both engines remain readable as plain text. A missing span means “not coloured yet” or “no rule claimed this text”; it is not a rendering error.

The public seam is `Engine`, `Grammar`, `Palette`, `Span`, `Style`, `Syntax`, `SyntaxRequest`, and `SyntaxResponse`. Engine-specific names stay under `src/engine/`.
