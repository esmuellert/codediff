# libvscode-diff

The bundled C implementation of codediff's VS Code-compatible line diff engine. It includes the C sources, public headers, tests, version file, CMake build, and vendored `utf8proc` source.

## Build

```sh
cmake -S . -B build -DENABLE_OPENMP=OFF
cmake --build build
ctest --test-dir build --output-on-failure
```

OpenMP is optional in the standalone CMake build. The Cargo build used by `codediff` compiles the engine into a static archive with OpenMP disabled.

## Origin

The initial tree came from `esmuellert/codediff.nvim` v2.60.0 at commit `dc38f0b8a2ba8cc198cc024f3abe887341788820`. It is a C port of VS Code's diff implementation. Parity corrections for VS Code commit `08d4889f9ec4a1685d257b9b95de036c8e1ce1e5` were incorporated before this directory became canonical.

This directory is maintained in this repository. It has no build or update dependency on the Neovim plugin.
