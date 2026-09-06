//! Original vs modified: which version of a file.

/// A file version used by a diff lookup.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum DiffVersion {
    Original,
    Modified,
}

impl DiffVersion {
    /// Both, original first — for a caller that must handle each in turn.
    pub const BOTH: [DiffVersion; 2] = [DiffVersion::Original, DiffVersion::Modified];

    /// The other one.
    pub fn other(self) -> Self {
        match self {
            DiffVersion::Original => DiffVersion::Modified,
            DiffVersion::Modified => DiffVersion::Original,
        }
    }
}
