#![doc = include_str!("../README.md")]
//!
//! ---
//!
//! Admission criterion: does this ask a version control system a question, or
//! model its answer?
//!
//! Two layers. The trait below is in the reviewer's terms and names no git
//! concept; [`git`] keeps git's own vocabulary and is the only place `git` runs.

mod change;
mod error;
pub mod git;

pub use change::{Change, ChangedFile, RelPath, Repo};
pub use error::{Error, Result};
pub use git::Git;

/// What a reviewer needs from a version control system.
///
/// Deliberately small and deliberately neutral: no index, no `HEAD`, no blob
/// and no object id, because a system need not have any of them — jj has no
/// staging area at all. What "before" means is decided when a backend is
/// constructed, not by this trait.
///
/// Capabilities that only some systems have — staging, history — belong in
/// separate traits, so a backend that lacks one fails to compile rather than
/// returning "unsupported" at runtime.
pub trait Vcs {
    /// The repository being read.
    fn repo(&self) -> &Repo;

    /// Every file that differs between the two sides.
    fn changed_files(&mut self) -> Result<Vec<ChangedFile>>;

    /// The file's content before the change. `None` when it did not exist.
    ///
    /// Takes the whole [`ChangedFile`] rather than a path so that a move reads
    /// its old path without the caller having to know that rule.
    fn before(&mut self, file: &ChangedFile) -> Result<Option<Vec<u8>>>;

    /// The file's content after the change. `None` when it no longer exists.
    fn after(&mut self, file: &ChangedFile) -> Result<Option<Vec<u8>>>;
}
