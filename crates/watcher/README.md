# watcher

Reports repository changes as `watcher::Refresh` values. It does not run Git, rescan files, or decide which UI entries to redraw.

`subscribe` resolves the repository's worktree-specific and common Git directories, installs watches, and starts one event worker. Worktree directories respect Git ignore rules; Git metadata watches cover the index, `HEAD`, refs, and packed refs without walking object storage.

Raw notify events are filtered and coalesced. A normal burst is emitted after 50 ms of quiet and is capped at 250 ms during continuous activity. Queue overflow or a backend rescan request conservatively sets all refresh bits.

```rust
Refresh {
    worktree: bool,
    index: bool,
    head: bool,
    refs: bool,
}
```

## JSONL helper

The optional `codediff-watcher` binary exposes the same stream for editor integrations:

```sh
codediff-watcher /absolute/path/to/repository
codediff-watcher --version
```

The first line is written only after the initial watches are installed:

```json
{"type":"ready","protocol":1,"binary_version":"0.23.0"}
```

Each later line is one coalesced invalidation. Every line is flushed immediately:

```json
{"type":"refresh","worktree":true,"index":false,"head":false,"refs":false}
```

Stdout contains only protocol messages. Startup errors and process failures go to stderr; a startup failure emits no `ready` message and exits non-zero. The consumer decides when to run Git and how to update its state.
