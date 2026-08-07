//! One visible line, as facts rather than as text.
//!
//! A row says *what is on this line and where it sits*, never what it looks
//! like. `▾`, `│ `, `+4` and `M` are choices only something with a terminal
//! can make, and they are made in `ui`, beside the theme that colours them.
//!
//! That is the division `align` already keeps: it reports that a view line is
//! a gap, and never that a gap is drawn `╱`. This crate used to hold both
//! halves, which is why the one piece of it that was general — fitting a row
//! into a narrow pane — could not be reused by anything else. See D65.

use file_types::{ChangeType, Stats};

use crate::node::NodeId;

/// One visible line of the explorer.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Row {
    /// Which node this row shows, so a key press has something to act on.
    pub node: NodeId,
    /// Where this row sits in the tree, or `None` for a section heading.
    ///
    /// A heading is what the tree hangs from rather than a line in it, so it
    /// has no indent to describe. An empty [`Guides`] would say "at the top
    /// level", which is a different statement.
    pub guides: Option<Guides>,
    pub content: Content,
}

/// Where a row sits among its siblings, at every level above it.
///
/// The indent is described rather than drawn, because a guide at a given depth
/// means *an ancestor at that depth has more children after it* — a fact about
/// the walk, not a property of the node, and not a character until something
/// with a screen says so.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Guides {
    /// For each level between the section and this row, whether that ancestor
    /// was the last of its siblings.
    pub ancestors: Vec<bool>,
    /// Whether this row is the last of its own siblings.
    pub is_last: bool,
}

/// What one row is.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Content {
    /// A section: everything one comparison of two revisions produced.
    Heading {
        title: String,
        /// How many files it holds, however deeply.
        files: usize,
        /// Their total, or `None` when the reader has turned counts off or
        /// nothing under it was counted.
        stats: Option<Stats>,
    },
    /// A directory, and whether it is open.
    ///
    /// The name may be several directories deep — `deep/only/one/chain` —
    /// when none of them had a choice in it.
    Directory { name: String, open: bool },
    /// One changed file.
    File {
        name: String,
        /// Where it came from, when it moved.
        moved_from: Option<String>,
        /// What it gained and lost, or `None` when the reader has turned
        /// counts off or git reported none.
        stats: Option<Stats>,
        /// What happened to it.
        ///
        /// The letter git writes for this is a spelling, and lives with the
        /// theme that colours it — the same split as `syntax::Group` against
        /// `theme::Code`, and the reason neither can silently take on the
        /// other's job.
        change: ChangeType,
    },
}
