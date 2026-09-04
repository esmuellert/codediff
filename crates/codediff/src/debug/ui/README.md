# UI gallery

The gallery is part of the compiled `codediff` binary. It mounts production UI
components with deterministic data; it does not copy their rendering code.

```text
catalog.rs       explicit group order and lookup
catalog_rows.rs  filtering, row navigation, and row styles
browser.rs       searchable Catalog component
browse.rs        one terminal session that switches Catalog/Preview trees
chrome.rs        shared two-line menu with semantic colour roles
component.rs     Preview shell around a production component
definition.rs    StoryDefinition and typed StoryFixture
session.rs       direct-story Harness and terminal hosts
stories/         definitions grouped by production component
fixtures/        reusable typed data builders
```

With no ID, `browse.rs` keeps one terminal session open while replacing the
Catalog or Preview tree. Each preview receives a generation number; responses
from a worker belonging to an older preview are ignored. Switching or resetting
therefore remounts hooks and viewport state without allowing stale fixture data
to enter the new component.

## Adding a story

1. Add a builder function and `StoryDefinition` in the matching `stories/`
   module.
2. Build data through `fixtures/`; two-sided text must pass through the real
   diff and alignment pipeline.
3. Add an independent expected-content row in `tests/stories.rs`.
4. Run the story directly and through the PTY tests.

Do not put expected output in `StoryDefinition`: the catalog and its test
oracle must not change together.

Keep canonical stories small enough to isolate one behaviour. Add a separate
edge story when several difficult inputs need to be inspected together. Use
`long_rust_constant` for horizontal-overflow scenarios; it guarantees at least
512 terminal cells rather than relying on source byte length.
