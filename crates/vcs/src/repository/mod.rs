//! The public API: open, list changes, count lines, read a file.
//!
//! ```text
//! mod.rs    Repository — open, repo_path, and private helpers
//! list.rs   get_changed_files, get_line_stats
//! read.rs   get_file_content, get_raw_content
//! ```
//!
//! Split by what the caller is asking for. "What changed?" and "Show me this
//! file" use different git commands, hold different state, and are called at
//! different times. That is two reasons to change, so two files.

mod diff_type;
pub(crate) mod list;
mod read;

pub use diff_type::DiffType;
pub use list::LineStats;

use std::path::Path;

use file_types::Revs;

use crate::git::{cat_file, rev_parse};
use crate::repo::Repo;

/// An open repository.
#[derive(Debug)]
pub struct Repository {
    repo: Repo,
    /// Resolved once on first use — a mid-review commit must not split naming.
    revs: Option<Revs>,
    /// The `cat-file --batch` child, opened on first use.
    blobs: Option<cat_file::Batch>,
}

impl Repository {
    /// Opens the repository containing `path`.
    ///
    /// `path` is a place to start looking, not the root: the backend discovers
    /// that, and every path built afterwards is relative to what it found.
    pub fn open(path: &Path) -> crate::Result<Self> {
        Ok(Self {
            repo: rev_parse::find_repo(path)?,
            revs: None,
            blobs: None,
        })
    }

    /// Where the repository is.
    ///
    /// The root is what paths are relative to; the control directory is what a
    /// file watcher needs to notice a branch switch.
    pub fn repo_path(&self) -> &Repo {
        &self.repo
    }

    /// What the two sides of the working-tree comparison are, resolved.
    pub(crate) fn revs(&mut self) -> crate::Result<Revs> {
        if self.revs.is_none() {
            let commit = rev_parse::resolve_or_empty(&self.repo, "HEAD")?;
            self.revs = Some(Revs::worktree_against(commit));
        }
        Ok(self.revs.clone().expect("just resolved"))
    }
}
