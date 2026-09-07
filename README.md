# codediff

A terminal reviewer for Git changes. It is aimed at the workflow where an agent edits a repository and a human checks the result. The project is still under active development.

`codediff` is not an AI agent and does not call a language model. It reads Git state, computes diffs, and displays them; it also supports staging and unstaging from the file explorer.

## What it does

- lists unstaged, staged, untracked, deleted, renamed, and conflicted files;
- shows text changes side by side, with line and character decorations;
- shows added, deleted, and untracked files as one-sided text;
- applies syntax colours with Tree-sitter where available and TextMate grammars elsewhere;
- refreshes the file list and the selected file after repository changes;
- stages or unstages a file or directory with the explorer's space key.

Binary files are identified and are not sent through the line diff. Conflict entries are listed and marked; the program does not resolve merges.

## Build and run

Rust and a C compiler are required. The pinned toolchain is in `rust-toolchain.toml`.

```sh
cargo build --release
cd /path/to/a/git/repository
/path/to/codediff/target/release/codediff
```

The normal command opens the current repository. A single path argument narrows the file list to that path. The hidden debug commands are useful when inspecting one layer without opening the terminal UI:

```sh
codediff doctor
codediff debug status -v
codediff debug diff-file path/to/file
codediff debug ui --list
codediff debug ui side-by-side/replacement --snapshot --width 100 --height 24
```

## Basic controls

In the explorer: `j`/`k` or the arrow keys move, `Enter` opens a file or folds a directory, `i` switches tree/list mode, `Space` stages or unstages the selected file or directory, and the right arrow focuses the diff.

In a diff: `j`/`k` or the arrow keys scroll vertically, `h`/`l` scroll horizontally, `0` moves to the beginning, `$` moves to the end, and the left arrow returns to the explorer. The mouse and wheel are also supported. `q` exits.

## Project notes

The current architecture, crate map, worker flow, and verification commands are in [`docs/README.md`](docs/README.md). Historical plans and superseded designs are kept under [`docs/archive/`](docs/archive/).

The project is licensed under MIT. Third-party notices are in [`ATTRIBUTION.md`](ATTRIBUTION.md).
