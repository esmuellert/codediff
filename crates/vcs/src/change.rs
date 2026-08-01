//! What changed, in the terms a reviewer thinks in.
//!
//! Nothing here names a git concept. There is no index, no `HEAD`, no blob and
//! no object id, because a version control system need not have any of them —
//! jj has no staging area at all. Git's own vocabulary lives in
//! [`crate::git`] and stops there.

use std::path::{Path, PathBuf};

/// A path relative to the repository root, in the backend's own spelling.
///
/// Git reports forward slashes on every platform, relative to the root and
/// never to the current directory. Keeping that in one type stops a Windows
/// backslash or a `../` from leaking into a lookup key.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct RelPath(String);

impl RelPath {
    pub fn new(path: impl Into<String>) -> Self {
        Self(path.into())
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }

    /// Resolved against a repository root.
    pub fn to_absolute(&self, root: &Path) -> PathBuf {
        root.join(&self.0)
    }

    /// The final component, for display.
    pub fn file_name(&self) -> &str {
        self.0.rsplit('/').next().unwrap_or(&self.0)
    }
}

impl std::fmt::Display for RelPath {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

/// What happened to a file between the two sides being compared.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Change {
    /// Exists only on the after side.
    Added,
    /// Exists on both sides, with different content.
    Modified,
    /// Exists only on the before side.
    Deleted,
    /// The same content under a different path.
    Moved,
    /// Not under version control at all, so there is no before side.
    Untracked,
    /// Left unresolved by a merge. Reported so it is not silently missing;
    /// resolving one means editing the file, which this tool does not do.
    Conflicted,
}

impl Change {
    /// True when only one side exists, so there is nothing to pair against.
    pub fn is_one_sided(self) -> bool {
        matches!(self, Change::Added | Change::Deleted | Change::Untracked)
    }
}

/// One file that differs between the two sides.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ChangedFile {
    /// Where the file is on the after side — or where it was, if deleted.
    pub path: RelPath,
    /// Where it was on the before side, when that differs.
    pub previous_path: Option<RelPath>,
    pub change: Change,
    /// How alike the two paths are, 0–100, when the file moved.
    pub similarity: Option<u8>,
}

impl ChangedFile {
    /// The path to read on the before side, which is the old one for a move.
    pub fn before_path(&self) -> &RelPath {
        self.previous_path.as_ref().unwrap_or(&self.path)
    }

    pub fn is_conflicted(&self) -> bool {
        self.change == Change::Conflicted
    }

    pub fn is_moved(&self) -> bool {
        self.change == Change::Moved
    }
}

/// An open repository.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Repo {
    /// The working root — what paths are relative to.
    pub root: PathBuf,
    /// Where the backend keeps its own state. The file watcher needs it to
    /// notice a branch switch, and it is not always inside `root`.
    pub control_dir: PathBuf,
}
