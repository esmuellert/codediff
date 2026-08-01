//! The repository being read.

use std::path::PathBuf;

/// An open repository.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Repo {
    /// The working root — what paths are relative to.
    pub root: PathBuf,
    /// Where the backend keeps its own state. The file watcher needs it to
    /// notice a branch switch, and it is not always inside `root`.
    pub control_dir: PathBuf,
}
