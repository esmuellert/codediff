# Current project notes

This file describes the implementation that is currently in the repository. It is intentionally short. Older plans, decisions, measurements, and framework specifications are historical material under [`archive/`](archive/).

## Runtime flow

```text
codediff
  └─ ui::main
      ├─ files worker       Git status and line statistics
      ├─ diff worker        Git content → VS Code diff → alignment
      ├─ syntax worker      syntax spans for visible file lines
      ├─ watcher             filesystem and Git-directory invalidations
      └─ loom tree           event routing, layout, components, painting
```

The UI thread owns the terminal and the Loom tree. Workers perform blocking Git, diff, syntax, and filesystem work. Results return through the application's event channel; stale file and repository responses are ignored by the receiving services.

Opening a file follows this path:

1. `FilesService` receives the list from `pipeline::files`.
2. `Explorer` puts the selected `file_types::File` into shared context.
3. `DiffService` asks `pipeline::diff` to read both sides.
4. `pipeline` classifies content, computes the diff with `vscode-diff`, and builds an `align::Alignment`.
5. `DiffViewer` renders either `SideBySide` or `SingleFile`.
6. Syntax requests are made only for the visible lines and are installed as worker responses.

The watcher sends refresh categories, not a list of changed paths. Explorer rescans its file list; the selected diff is reloaded when its mutable side is affected.

## Workspace crates

| crate | responsibility |
|---|---|
| `codediff` | command-line entry point, debug commands, doctor, component stories |
| `ui` | application components, services, navigation, themes, terminal integration |
| `loom` / `loom-macros` | terminal component tree, hooks, layout, paint, events, and RSX macros |
| `pipeline` | background file-list and file-content pipelines |
| `vcs` | Git commands, status parsing, revision reads, line statistics, staging |
| `watcher` | repository filesystem invalidations and the watcher helper binary |
| `syntax` | language detection, Tree-sitter/TextMate engines, syntax spans and cache |
| `align` | line pairing, fillers, hunk and character decorations |
| `line-index` | byte, UTF-16, grapheme, and terminal-cell coordinates |
| `file-types` | files, paths, revisions, content classification, and comparison modes |
| `diff-types` | owned data structures returned by the C diff engine |
| `vscode-diff` / `vscode-diff-sys` | safe Rust wrapper and raw FFI for the bundled C engine |
| `channel` | typed worker request/response helpers |
| `fixtures` | deterministic Git repositories for tests and manual checks |

The dependency direction is kept acyclic. Pure model crates do not invoke Git or the terminal; `xtask lint-arch` checks the repository-specific boundaries.

## Git comparisons

The ordinary worktree request contains separate entries for the two Git comparisons that `git status` describes:

- unstaged: index → worktree;
- staged: `HEAD` → index.

A conflicted entry is represented as a comparison from the merge stage selected by the backend to the worktree. The explorer groups files by their revision pair. A one-sided file has no comparison partner and is rendered in one pane.

The explorer can call `git add` and `git reset HEAD -- <path>` through `vcs`. The rest of the reviewer is observational; it does not edit file contents or resolve conflicts.

## Verification

Useful commands from the repository root:

```sh
cargo fmt --all --check
cargo test --workspace --no-fail-fast
cargo xtask lint-size
cargo xtask lint-arch
cargo xtask verify-oracle
cargo run -p xtask -- fixture-repo /tmp/codediff-fixture
```

The fixture command removes and recreates its destination. Use a disposable path.

For the bundled C implementation:

```sh
cmake -S libvscode-diff -B target/libvscode-diff-cmake -DENABLE_OPENMP=OFF
cmake --build target/libvscode-diff-cmake --parallel
ctest --test-dir target/libvscode-diff-cmake --output-on-failure
```

The component gallery provides deterministic text frames and PTY coverage for the production UI:

```sh
codediff debug ui --list
codediff debug ui explorer/tree --snapshot --width 100 --height 24
```
