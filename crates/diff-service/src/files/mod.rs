//! One request for a set of changed files.
//!
//! ```ignore
//! let files = files::get_files(&files::Request::worktree(root))?;
//! ```

use std::path::PathBuf;

use anyhow::{Context, Result};
use file_types::File;
use vcs::Repository;
pub use vcs::RevisionRange;

/// One request for a set of files.
#[derive(Clone)]
pub struct Request {
    /// Where to start looking. Not the root — the backend discovers that, and
    /// every path built afterwards is relative to what it found.
    pub repo: PathBuf,
    /// Which paths to narrow to, empty being everything.
    pub pathspec: Vec<String>,
    pub revision_range: RevisionRange,
}

impl Request {
    /// The ordinary question: what have I changed and not committed.
    pub fn worktree(repo: impl Into<PathBuf>) -> Self {
        Self::new(repo, RevisionRange::Worktree)
    }

    pub fn new(repo: impl Into<PathBuf>, revision_range: RevisionRange) -> Self {
        Self {
            repo: repo.into(),
            pathspec: Vec::new(),
            revision_range,
        }
    }

    pub fn with_pathspec(mut self, pathspec: Vec<String>) -> Self {
        self.pathspec = pathspec;
        self
    }
}

/// Every file the request finds, each with what it gained and lost.
pub fn get_files(request: &Request) -> Result<Vec<File>> {
    tracing::info!("listing files");
    let mut repository = Repository::open(&request.repo).context("opening a repository")?;
    let files = repository
        .get_changed_files(&request.revision_range, &request.pathspec)
        .context("listing changed files and line counts")?;
    tracing::info!(count = files.len(), "listed files");
    Ok(files)
}
