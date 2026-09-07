//! Comparison modes accepted by the Git backend.

/// Which comparison a review is of.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DiffType {
    /// Worktree changes: index → worktree and commit → index.
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
