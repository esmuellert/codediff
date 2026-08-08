//! Original vs modified: which version of a file.

/// Which version of a file a line, a column or a lookup refers to.
///
/// Not `Left`/`Right` — inline view puts both versions in one column.
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
