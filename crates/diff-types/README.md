# diff-types

Owned Rust data structures for a computed diff. The crate has no build script, C dependency, or I/O.

```text
LinesDiff
├── DetailedLineRangeMapping
│   ├── LineRange          1-based, end-exclusive line range
│   └── RangeMapping       character mapping inside a changed range
│       └── CharRange      UTF-16 offsets used by VS Code
└── MovedText              a moved block
```

The names and coordinate conventions mirror the bundled VS Code-compatible C engine. `vscode-diff` converts the FFI result into these owned values; `align` consumes them without depending on the FFI crate.
