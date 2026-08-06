//! Which way a file under review is shown.
//!
//! One enum for a fork that used to be spelled four ways — an
//! `Option<DiffType>`, an `Option<&Alignment>`, an `Option<DiffVersion>` and
//! a return type of its own — each of which said "single file" by being
//! absent. A third answer is not an absent one, which the keymap had already
//! worked out for itself:
//!
//! ```text
//! /// *Not* `Option<DiffType>`: the explorer is a third answer, not an
//! /// absent one.
//! ```
//!
//! It lives here, in the crate every layer names, so that the pipeline that
//! produces a file, the pairing that describes it and the interface that draws
//! it all say it with the same word.

/// Which way a file under review is shown.
///
/// The three are not variations on a theme: they produce different view-line
/// counts from the same file, so a position in one is meaningless in another.
/// That is why the buffer showing a file is a different buffer per type, and
/// why switching between them has to translate through a line number.
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
    /// The other way of reading the same two versions, which is what a toggle
    /// asks for.
    ///
    /// A single file has no other: there is one version, and no second way to
    /// arrange it. The toggle key therefore does nothing rather than
    /// pretending, which is what it already did when there was no diff on
    /// screen.
    pub fn other(self) -> Self {
        match self {
            Self::SideBySide => Self::Inline,
            Self::Inline => Self::SideBySide,
            Self::Single => Self::Single,
        }
    }

    /// Whether this shows two versions paired against each other.
    pub fn is_paired(self) -> bool {
        !matches!(self, Self::Single)
    }
}
