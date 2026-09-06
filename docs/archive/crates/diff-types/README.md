# diff-types

What a computed diff *is*. Not how to compute one.

Six structs, no dependencies, no build script, no `unsafe`:

```text
LinesDiff                    the whole result
├── DetailedLineRangeMapping one changed block
│   ├── LineRange            1-based, end-exclusive
│   └── RangeMapping         the characters within it that differ
│       └── CharRange        1-based, in UTF-16 code units
└── MovedText                a block that moved rather than being rewritten
```

## Why it is its own crate

`align` pairs up two files, line by line. It is pure, performs no IO, and is tested with
`proptest`. It also never calls the diff engine — it is *handed* a result and works out
where the fillers go.

But it has to name the result, and while these structs lived in `vscode-diff`, naming them
meant depending on the crate that links the C engine:

```text
align → vscode-diff → vscode-diff-sys → cc → libvscode-diff.a
```

So `cargo test -p align` compiled C. Now:

```text
align   ─┐
         ├→ diff-types          (no C anywhere)
vscode-diff → vscode-diff-sys → cc
```

`cargo xtask lint-arch` refuses to let `align` reach the engine again.

## The names are the engine's

`DetailedLineRangeMapping` is not a name anyone would invent. It is what the C header
calls it, which is what VSCode calls it, and matching them means a question about our
behaviour can be answered by reading their source. See
[D20](../../docs/plan/05-decisions.md#d20).

That the names are engine-shaped is not a reason for the structs to live *in* the engine's
crate. Nothing here mentions C, and a second engine — a pure-Rust fallback, or a WASM
build where `cc` cannot run — would produce these same values.

## Conventions, inherited from the engine

| | |
|---|---|
| lines | **1-based**, ranges **end-exclusive** |
| columns | **1-based**, counted in **UTF-16 code units**, not bytes |
| an empty `LineRange` | meaningful — it marks where text was inserted or removed on the other side |

The UTF-16 columns are why `line-index` exists: the engine counts the way JavaScript does,
and a terminal does not.
