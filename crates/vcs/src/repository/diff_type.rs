//! What to compare, in the reviewer's words.
//!
//! Five ways, and each is one command to the backend. A new one is an arm here
//! and an arm in `git::plan` — nothing above this crate learns about it, and
//! nothing below is told which the reader picked.
//!
//! **Revisions are held as the reader typed them** — `HEAD~3`, `main`, a tag —
//! not as ids. Resolving needs a repository, and this has to be nameable
//! without one.
//!
//! Named `DiffType` as [`file_types::DiffType`] is, and the two do not
//! collide: that one is how a file is *read* — two columns, one column, alone
//! — and this is what is being compared. The crate in front of the name says
//! which.

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
