# syntax

Says what a piece of text *is* — a keyword, a string, a comment — never what colour it
should be.

Real highlighting arrives at S11. Until then `Plain` returns nothing, which is not a
placeholder to be deleted: highlighting is too slow to run on the render path, so a
renderer must already cope with having no spans yet. This is that state, made explicit and
testable.

## Why the token list is short

Ten kinds, not forty. A highlighter that distinguishes forty maps them onto these, so a
theme has a fixed set to colour and adding a language cannot add a colour.

## What it deliberately does not know

Colour. A `Token` is `Keyword`, not `#cba6f7`; `ui` owns the mapping. That is what
makes the engine swappable — and `cargo xtask lint-arch` enforces it, by refusing to let
the name of a syntax engine appear anywhere outside `crates/syntax/src/engine`.
