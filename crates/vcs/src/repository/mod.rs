//! The whole surface: open a repository, ask what changed, read a file.
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

use std::path::Path;

use file_types::Revs;

use crate::git::{cat_file, rev_parse};
use crate::repo::Repo;

/// An open repository.
///
/// Holds what a session accumulates — what has been resolved, which child
/// process is open — so that everything below can be a free function over a
/// [`Repo`] and none of it has to be told twice.
#[derive(Debug)]
pub struct Repository {
    repo: Repo,
    /// Resolved on first use, so a list-only run never pays for the extra
    /// process — and resolved **once**, so a commit made while a review is
    /// open cannot leave half its files named against one `HEAD` and half
    /// against another.
    revs: Option<Revs>,
    /// Opened on first use, so a list-only run never pays for the child.
    blobs: Option<cat_file::Batch>,
}

impl Repository {
    /// Opens the repository containing `path`.
    ///
    /// `path` is a place to start looking, not the root: the backend discovers
    /// that, and every path built afterwards is relative to what it found.
    pub fn open(path: &Path) -> crate::Result<Self> {
        Ok(Self {
            repo: rev_parse::discover(path)?,
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
