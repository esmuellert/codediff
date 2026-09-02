# libvscode-diff

The canonical C implementation of codediff's VS Code-compatible diff engine.
It contains its own source, public headers, tests, version, CMake project, and
bundled utf8proc dependency.

## Build

```sh
cmake -S . -B build -DENABLE_OPENMP=OFF
cmake --build build
ctest --test-dir build --output-on-failure
```

OpenMP is optional. Enabling it uses the platform OpenMP runtime when one is
available; disabling it leaves only the normal platform C and math libraries.

## Origin

The initial C tree came from `esmuellert/codediff.nvim` v2.60.0 at commit
`dc38f0b8a2ba8cc198cc024f3abe887341788820`. It is a C port of VS Code's diff
implementation. Parity corrections for VS Code commit
`08d4889f9ec4a1685d257b9b95de036c8e1ce1e5` were incorporated when this
directory became canonical.

The initial canonical revision incorporates two corrections found by comparing
real Git history with that VS Code build:

- character heuristics trim and inspect UTF-16 elements rather than narrowed C bytes
- Myers diagonals preserve `FastInt32Array` out-of-capacity behavior

That history is provenance only. This directory is maintained here and has no
build or update dependency on `codediff.nvim`.
