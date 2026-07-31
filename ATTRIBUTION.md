# Attribution

Third-party code compiled into the `codediff` binary, and the notices their licences
require us to carry.

This is a narrower list than the upstream Neovim plugin's, which also covers Lua
dependencies, vendored Neovim internals and documentation assets — none of which are
present here.

Everything below is compiled by `crates/vscode-diff-sys/build.rs` from
`vendor/libvscode-diff/`.

---

## Derivative Works

### Microsoft Visual Studio Code

**License**: MIT License
**Copyright**: Copyright (c) Microsoft Corporation
**Source**: https://github.com/microsoft/vscode

The diff computation in this project is a C port of VSCode's
`defaultLinesDiffComputer`. The algorithm, data structures and optimisation heuristics
are derived from VSCode's TypeScript source.

Components ported:

- Myers diff algorithm —
  `src/vs/editor/common/diff/defaultLinesDiffComputer/algorithms/myersDiffAlgorithm.ts`
- Dynamic programming algorithm —
  `src/vs/editor/common/diff/defaultLinesDiffComputer/algorithms/dynamicProgrammingDiffing.ts`
- Line-level optimisation heuristics —
  `src/vs/editor/common/diff/defaultLinesDiffComputer/heuristicSequenceOptimizations.ts`
- Character-level refinement —
  `src/vs/editor/common/diff/defaultLinesDiffComputer/defaultLinesDiffComputer.ts`
- Range mapping data structures — `src/vs/editor/common/diff/rangeMapping.ts`

```
MIT License

Copyright (c) Microsoft Corporation

Permission is hereby granted, free of charge, to any person obtaining a copy
of this software and associated documentation files (the "Software"), to deal
in the Software without restriction, including without limitation the rights
to use, copy, modify, merge, publish, distribute, sublicense, and/or sell
copies of the Software, and to permit persons to whom the Software is
furnished to do so, subject to the following conditions:

The above copyright notice and this permission notice shall be included in all
copies or substantial portions of the Software.

THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND, EXPRESS OR
IMPLIED, INCLUDING BUT NOT LIMITED TO THE WARRANTIES OF MERCHANTABILITY,
FITNESS FOR A PARTICULAR PURPOSE AND NONINFRINGEMENT. IN NO EVENT SHALL THE
AUTHORS OR COPYRIGHT HOLDERS BE LIABLE FOR ANY CLAIM, DAMAGES OR OTHER
LIABILITY, WHETHER IN AN ACTION OF CONTRACT, TORT OR OTHERWISE, ARISING FROM,
OUT OF OR IN CONNECTION WITH THE SOFTWARE OR THE USE OR OTHER DEALINGS IN THE
SOFTWARE.
```

---

## Bundled Dependencies

### utf8proc

**License**: MIT "expat" License, plus Unicode data terms
**Copyright**: Copyright (c) 2014-2021 Steven G. Johnson, Jiahao Chen, Tony Kelman,
Jonas Fonseca, and other contributors
**Source**: https://github.com/JuliaStrings/utf8proc
**Location**: `vendor/libvscode-diff/vendor/`
**Purpose**: UTF-8 processing, and conversion of byte offsets to UTF-16 code unit
offsets so that column positions match VSCode's

Full license text:
[vendor/libvscode-diff/vendor/utf8proc_LICENSE.md](vendor/libvscode-diff/vendor/utf8proc_LICENSE.md)

---

## Maintaining this file

Written by hand rather than copied from upstream, because attribution describes what
*this* binary contains.

`cargo xtask sync-c` lists the bundled third-party sources it finds, so a new one
cannot arrive here unnoticed.
