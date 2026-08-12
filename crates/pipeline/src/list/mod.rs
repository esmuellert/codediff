//! One request for a set of changed files.
//!
//! ```ignore
//! let files = list::get_files(&list::Request::worktree(root))?;
//! ```

mod worker;

pub use worker::ListWorker;

use std::path::PathBuf;

use anyhow::{Context, Result};
use file_types::File;
use vcs::{DiffType, Repository};

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

/// Every file the request finds, each with what it gained and lost.
pub fn get_files(request: &Request) -> Result<Vec<File>> {
    tracing::info!("listing files");
    let mut repository = Repository::open(&request.repo).context("opening a repository")?;
    let changes = repository
        .get_changed_files(&request.diff_type, &request.pathspec)
        .context("listing changed files")?;

    let counts = repository
        .get_line_stats(&request.diff_type, &request.pathspec)
        .unwrap_or_default();

    let files: Vec<File> = changes
        .into_iter()
        .map(|file| match counts.of(&file) {
            Some(stats) => file.set_stats(stats),
            None => file,
        })
        .collect();
    tracing::info!(count = files.len(), "listed files");
    Ok(files)
}
