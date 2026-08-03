//! Which of the two.

/// Which version of a file a line, a column or a lookup refers to.
///
/// Deliberately not `Left` and `Right`. Those are places on a screen, and
/// inline view puts both versions in one column; a name that means a place
/// could not describe it.
///
/// Not a git revision either — `Original` is whatever was being compared
/// against, which may be HEAD, the index, or another commit. Which of those it
/// is belongs to whatever built the comparison.
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
