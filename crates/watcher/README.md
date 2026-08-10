# watcher

Watches a git repository for changes and reports what needs refreshing.

## What is here

```text
lib.rs       re-exports: Refresh, Watcher, start
refresh.rs   Refresh — a bitset of what changed (worktree | index | head | refs)
filter.rs    path → Refresh — pure logic, all filtering decisions
scope.rs     which directories to hand notify (platform-aware)
watch.rs     the debouncer, the callback, the handle
```

## How it works

Two watch scopes feed one debouncer:

1. **Worktree** — every non-ignored directory (Linux: one `NonRecursive` watch each;
   macOS/Windows: one `Recursive` on the root). Enumerated with the `ignore` crate so
   `target/`, `node_modules/`, and anything in `.gitignore` is never watched at all.

2. **The git dirs** — `NonRecursive` on the worktree's own git dir (catches `index`, `HEAD`),
   plus `Recursive` on `refs/` in the shared one (catches branch moves, tags, stash), plus
   `NonRecursive` on the shared dir itself (catches `packed-refs`) when the two differ.

   They differ in a linked worktree: `.git` there is a *file* reading `gitdir: <path>`, and
   that directory's `commondir` file names the original `.git`. A plain repository has
   neither file, and both are `.git` itself.

The `notify-debouncer-full` collapses kernel-level bursts into one batch per 50 ms. The
batch is handed to `filter::get_refresh`, which:

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

The filter runs on `notify`'s internal thread. Each event costs a few string prefix
comparisons — microseconds. The 50 ms debounce means at most 20 batches per second under
sustained activity. On Linux, ignored directories are never handed to inotify, so build
output generates zero kernel events regardless of how many files it writes.
