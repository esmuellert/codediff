//! What set of files to show, and where they come from.
//!
//! Here rather than in the binary because two things build one: the command
//! line today, and the interface later — a reader changing what they are
//! comparing without leaving the review. `ui` can name this crate, and cannot
//! name the binary, so this is the lowest place both can reach.
//!
//! Nothing here runs git. These are the words for a question; `codediff`'s
//! list pipeline is what answers it.

use std::path::PathBuf;

/// One reader's question: which files, from where.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExplorerDiffRequest {
    /// Which repository. Absolute, so nothing below has to know where the
    /// reader was standing.
    pub repo: PathBuf,
    /// Paths to narrow the list to, as git spells a pathspec. Empty is
    /// everything.
    pub pathspec: Vec<String>,
    pub diff_type: ExplorerDiffType,
}

impl ExplorerDiffRequest {
    /// The ordinary question: what have I changed and not committed.
    pub fn worktree(repo: impl Into<PathBuf>) -> Self {
        Self::new(repo, ExplorerDiffType::Worktree)
    }

    pub fn new(repo: impl Into<PathBuf>, diff_type: ExplorerDiffType) -> Self {
        Self {
            repo: repo.into(),
            pathspec: Vec::new(),
            diff_type,
        }
    }

    pub fn with_pathspec(mut self, pathspec: Vec<String>) -> Self {
        self.pathspec = pathspec;
        self
    }
}

/// Which comparison the list is of.
///
/// **Revisions are held as the reader typed them** — `HEAD~3`, `main`, a tag —
/// not as ids. Resolving is the list pipeline's first stage, and doing it
/// here would mean this type could not be built without a repository to
/// resolve against.
///
/// Each variant is one git command. That is the whole point of the enum: a new
/// way to compare is a new arm and its arguments, and nothing downstream —
/// not the groups, not the explorer, not the interface — learns about it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ExplorerDiffType {
    /// What is not committed: two comparisons, the working tree against the
    /// index and the index against the commit.
    ///
    /// The only variant that yields more than one group, which is why it
    /// cannot be expressed as a pair of revisions.
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
