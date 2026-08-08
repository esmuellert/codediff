//! Which way a file under review is shown.

/// Which way a file under review is shown.
///
/// The three produce different view-line counts from the same file, so a
/// position in one is meaningless in another.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum DiffType {
    /// Both versions at once, a view line carrying a slot for each.
    #[default]
    SideBySide,
    /// One version per view line: what was deleted, then what replaced it.
    Inline,
    /// The one version there is.
    ///
    /// An added, untracked or deleted file exists on a single side, so there
    /// is nothing to pair it against and no empty column to draw. See D23.
    Single,
}

impl DiffType {
    /// The other layout for two versions. `Single` has no other.
    pub fn other(self) -> Self {
        match self {
            Self::SideBySide => Self::Inline,
            Self::Inline => Self::SideBySide,
            Self::Single => Self::Single,
        }
    }

    /// Whether this shows two versions paired against each other.
    pub fn is_diff(self) -> bool {
        !matches!(self, Self::Single)
    }
}
