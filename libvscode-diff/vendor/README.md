# Bundled third-party source

This directory contains the `utf8proc` source used by the bundled diff engine. Keeping it here makes the CMake and Cargo builds independent of a system utf8proc installation.

## utf8proc

- version in the checked-in header: **2.11.0**;
- source: <https://github.com/JuliaStrings/utf8proc>;
- license: MIT/Unicode terms, in `utf8proc_LICENSE.md`;
- files: `utf8proc.h`, `utf8proc.c`, and `utf8proc_data.c`.

The engine uses it for Unicode processing and UTF-8/UTF-16 coordinate handling. Update the source and its license together, then update `ATTRIBUTION.md` if the bundled contents change.
