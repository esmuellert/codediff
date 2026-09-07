# vcs

The Git backend. It answers four questions for the rest of the application:

- which files belong to a requested comparison;
- which revisions are on each side;
- how many lines changed, when Git can count them;
- what bytes or text each side contains.

`Repository` is the public entry point. The `git` modules run one Git command and parse Git's output; the repository layer translates that output into `file-types` values. The backend uses porcelain-v2 NUL-separated status records, forced rename detection, `git cat-file` for stored blobs, and the worktree for mutable files.

The ordinary worktree request yields separate files for unstaged and staged changes. It preserves both status codes for paths with changes in both the index and worktree. Reads apply checkout filters when a stored side is compared with the worktree.

`Repository::stage` runs `git add -- <path>`. `Repository::unstage` runs `git reset HEAD -- <path>`. These are the only content-adjacent mutations currently exposed by the application; the backend does not edit file contents.

The crate invokes the Git executable rather than linking a Git implementation, so Git's own ignore rules, filters, worktrees, and configuration decide what is visible.
