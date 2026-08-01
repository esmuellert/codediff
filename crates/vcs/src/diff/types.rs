//! What a per-file diff is made of.

use crate::path::RelPath;

/// What happened to a file between the two sides being compared.
///
/// Deliberately not called `Change`: the diff engine already reports *line*
/// level changes, and two meanings of the word in one pipeline is one too many.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DiffKind {
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

impl DiffKind {
    /// True when only one side exists, so there is nothing to pair against.
    pub fn is_one_sided(self) -> bool {
        matches!(
            self,
            DiffKind::Added | DiffKind::Deleted | DiffKind::Untracked
        )
    }
}

/// One file that differs between the two sides.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FileDiff {
    /// Where the file is on the after side — or where it was, if deleted.
    pub path: RelPath,
    /// Where it was on the before side, when that differs.
    pub previous_path: Option<RelPath>,
    pub kind: DiffKind,
    /// How alike the two paths are, 0–100, when the file moved.
    pub similarity: Option<u8>,
}

impl FileDiff {
    /// The path to read on the before side, which is the old one for a move.
    pub fn before_path(&self) -> &RelPath {
        self.previous_path.as_ref().unwrap_or(&self.path)
    }

    pub fn is_conflicted(&self) -> bool {
        self.kind == DiffKind::Conflicted
    }

    pub fn is_moved(&self) -> bool {
        self.kind == DiffKind::Moved
    }
}
