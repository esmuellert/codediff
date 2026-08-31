# watcher

Watches a git repository for changes and reports what needs refreshing.

## What is here

```text
lib.rs                         re-exports: Refresh, Subscription, subscribe
refresh.rs                     Refresh — worktree | index | head | refs
filter.rs                      path → Refresh; pure filtering logic
git_dirs.rs                    resolves worktree-specific and common Git directories
ignore_rules.rs                loads and detects changes to ignore rules
scope.rs                       computes and maintains the paths handed to notify
watch.rs                       the bounded debouncer, callback, and handle
src/bin/codediff-watcher.rs    JSONL helper process for editor integrations
```

## Helper protocol

Start one process per repository:

```text
codediff-watcher /absolute/path/to/repository
```

Stdout is UTF-8 JSON Lines. The first line is emitted only after every initial watch is
installed:

```json
{"type":"ready","protocol":1,"binary_version":"0.17.0"}
```

Each later line is one coalesced invalidation:

```json
{"type":"refresh","worktree":true,"index":false,"head":false,"refs":false}
```

Every line is flushed immediately. Stdout contains only protocol messages; process errors
are written to stderr. Startup failure produces no `ready` line and exits non-zero. A failed
stdout write also stops the process with a non-zero status. `codediff-watcher --version` prints the binary
version without starting a watcher.

## How it works

Two groups of watched paths feed one event worker:

1. **Worktree** — every non-ignored directory (Linux: one `NonRecursive` watch each;
   macOS/Windows: one `Recursive` on the root). The `ignore` crate keeps build output
   out; directory and ignore-rule changes update the watch scope while it is running.

2. **The git dirs** — `NonRecursive` on the worktree's own git dir (catches `index`, `HEAD`),
   plus `Recursive` on `refs/` in the shared one (catches branch moves, tags, stash), plus
   `NonRecursive` on the shared dir itself (catches `packed-refs`) when the two differ.

   They differ in a linked worktree: `.git` there is a *file* reading `gitdir: <path>`, and
   that directory's `commondir` file names the original `.git`. A plain repository has
   neither file, and both are `.git` itself.

`subscribe` succeeds only after resolving the Git directories and registering every
initial watch. Invalid repositories and partial watch installation are returned as errors.

The notify callback forwards raw events through a bounded queue. A worker emits one
`Refresh` after 50 ms of quiet, or after 250 ms under continuous activity. If the queue
fills, or the filesystem backend reports missed events, the worker conservatively refreshes
all state instead of trusting an incomplete event history.

- Skips `.lock` files (git renames them atomically to the real path)
- Skips git internals (`objects/`, `logs/`, `hooks/`, `lfs/`)
- Skips gitignored worktree paths (checked against the compiled `.gitignore`)
- Maps what remains to the four bits of `Refresh`

If the result is non-empty, one message is sent on the channel. The consumer decides when
and how often to act on it.

## What it does not do

- **Refresh the explorer** — that is the consumer's job (step 2).
- **Suppress during own git calls** — the consumer holds the counter, not the watcher.
- **Run `git status`** — it only reports *that* something changed, not *what* changed.
- **Watch recursively on Linux** — that would install inotify watches inside `target/`.

## Performance

The notify callback only sends events through a bounded channel; filtering and scope
maintenance run on the worker. Each batch has fixed size, and both threads block
while idle. On Linux, ignored directories are never handed to inotify, so build output
generates zero kernel events regardless of how many files it writes.
