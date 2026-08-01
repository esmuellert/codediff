//! Asking what differs, and reading the two sides of it.
//!
//! The capability every backend has. Named for what git and jj both call it,
//! and kept apart from the engine's `LinesDiff`: this one is **per file**, that
//! one is per line.

mod types;

pub use types::{DiffKind, FileDiff};

use crate::error::Result;
use crate::repo::Repo;

/// What a reviewer needs from a version control system.
///
/// Deliberately neutral: no index, no `HEAD`, no blob and no object id, because
/// a system need not have any of them — jj has no staging area at all. What
/// "before" means is decided when a backend is constructed, not here.
pub trait Diff {
    /// The repository being read.
    fn repo(&self) -> &Repo;

    /// Every file that differs between the two sides.
    fn files(&mut self) -> Result<Vec<FileDiff>>;

    /// The file's content before the change. `None` when it did not exist.
    ///
    /// Takes the whole [`FileDiff`] rather than a path so that a move reads its
    /// old path without the caller having to know that rule.
    fn before(&mut self, file: &FileDiff) -> Result<Option<Vec<u8>>>;

    /// The file's content after the change. `None` when it no longer exists.
    fn after(&mut self, file: &FileDiff) -> Result<Option<Vec<u8>>>;
}
