# ui

The application UI for `codediff`. It owns the terminal-facing component tree, navigation, themes, and bridges to background services.

## Shape

`loom::Tree` mounts `components::App`. The root provides `ui::Context`, which contains the repository, selected file, theme, and stable service handles.

```text
App
└─ UiProvider
   └─ Row
      ├─ Border ─ Explorer
      └─ Border ─ DiffViewer
                     ├─ SideBySide
                     └─ SingleFile
```

`Explorer` owns its loaded file list, selection, folds, and tree/list mode. `DiffViewer` owns the currently loaded `pipeline::diff::DiffContent` and chooses a two-sided or one-sided view. `Gutter`, `CodeText`, and `Filler` receive the values they paint as props.

## Services and workers

- `FilesService` requests `pipeline::files` and refreshes after watcher messages.
- `DiffService` requests `pipeline::diff` for the selected `file_types::File`.
- `SyntaxService` requests visible syntax spans and stores returned chunks.
- `VersionControlService` stages and unstages paths through `vcs`.
- `WatcherService` multicasts `watcher::Refresh` values.

The UI thread never runs Git or computes a diff while painting. The services deliver worker responses through the application event loop and reject responses for an old repository, file, or content generation where needed.

## Navigation

Explorer uses `j`/`k`, the arrow keys, `Enter`, `i`, `Space`, the right arrow, mouse clicks, and vertical wheel events. Diff views use `j`/`k`, `h`/`l`, `0`, `$`, the left arrow, mouse focus, and vertical or horizontal wheel events. `q` exits the application.

The current main view is side by side for two-sided content. Added, deleted, and untracked files are shown with `SingleFile`; they are not compared against an invented empty side. The alignment model also supports an inline projection, but the normal `DiffViewer` currently selects only `SideBySide` and `SingleFile`.

## Rendering rules

`components::cells` paints terminal cells from line-indexed text. Diff backgrounds are applied first; syntax supplies foregrounds and text modifiers. Tabs, wide graphemes, clipped clusters, control characters, and bidi controls are handled before the text reaches the terminal.

Themes are compiled into the UI. `Theme::from_environment` chooses between Catppuccin and basic dark/light themes from terminal environment variables. There is no current `--theme` CLI option.

## Gallery and tests

The production component gallery is available through the binary:

```sh
codediff debug ui --list
codediff debug ui explorer/tree --snapshot --width 100 --height 24
```

`loom::testing::Harness` renders components into a ratatui buffer. The codediff tests also exercise real PTYs for terminal takeover and restoration.
