//! What set of files to show, and where from.
//!
//! Moved out of the file list, which declared it and never used it: nothing in
//! that crate reads a request, because a request is what produces the files it
//! is handed. See D67.

use std::path::PathBuf;

use vcs::DiffType;

/// One request for a set of files.
pub struct Request {
    /// Where to start looking. Not the root — the backend discovers that, and
    /// every path built afterwards is relative to what it found.
    pub repo: PathBuf,
    /// Which paths to narrow to, empty being everything.
    pub pathspec: Vec<String>,
    pub diff_type: DiffType,
}

impl Request {
    /// The ordinary question: what have I changed and not committed.
    pub fn worktree(repo: impl Into<PathBuf>) -> Self {
        Self::new(repo, DiffType::Worktree)
    }

    pub fn new(repo: impl Into<PathBuf>, diff_type: DiffType) -> Self {
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
