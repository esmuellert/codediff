# file-types

The shared vocabulary for a file under review. It contains no Git commands, terminal code, or diff algorithm.

- `RepoPath` keeps Git's repository-relative spelling and the corresponding filesystem path.
- `File` records the path and revision present on each side of one comparison.
- `Rev`, `Revs`, and `Oid` identify worktree, index, conflict stages, and commits.
- `ChangeType` describes added, deleted, modified, moved, untracked, and conflicted entries.
- `FileContent` classifies a side as text, binary, or absent.
- `DiffVersion` names the original or modified side.
- `DiffType` names how a diff is displayed: side by side, inline, or single file.
- `Stats` carries optional added and removed line counts.

Absence is represented with `Option`, not an empty path or empty text. An empty tracked file is still present and can be compared; an absent side is what makes a file one-sided.

The backend supplies revision and backend-only status facts. The pipeline and UI use the same types rather than flattening a file into a display string.
