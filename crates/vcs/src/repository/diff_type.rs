//! What to compare.
//!
//! Five comparison modes, each mapping to one backend command. Revisions are
//! held as the reader typed them (not as ids), since resolving needs a
//! repository.

/// Which comparison a review is of.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DiffType {
    /// What is not committed: two comparisons, the working tree against the
    /// index and the index against the commit.
    ///
    /// The only one that yields more than one group, which is why it cannot be
    /// expressed as a pair of revisions.
    Worktree,
    /// One revision against the file on disk.
    Against(String),
    /// One revision against another.
    Between(String, String),
    /// Where a branch left another, against that branch — git's `a...b`.
    MergeBase(String, String),
    /// What is staged, against a revision.
    Staged(String),
}
