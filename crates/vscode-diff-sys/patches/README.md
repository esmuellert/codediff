# VS Code parity patches

`libvscode-diff` stays byte-for-byte identical to the tag recorded in
`vendor/UPSTREAM.lock`. `build.rs` applies the patches in this directory to
copies under `OUT_DIR` before compiling them.

The reference is VS Code commit
`08d4889f9ec4a1685d257b9b95de036c8e1ce1e5`, the same build used by
`cargo xtask verify-vscode`.

- `char-level-text.patch` matches
  `removeVeryShortMatchingTextBetweenLongDiffs`: it keeps the text as UTF-16
  elements while measuring JavaScript `trim()` and line-break behavior instead
  of narrowing each element to one C byte.
- `myers-typed-array.patch` matches `FastInt32Array`: an index outside the
  current typed-array capacity reads as `undefined`, making `Math.max` produce
  `NaN`; assigning that value back to `Int32Array` stores zero.

Both differences were reduced from real Git-history pairs. The public Rust
regressions live in `crates/vscode-diff/tests/end_to_end.rs`; the browser
comparison covers them as part of the historical sample.
