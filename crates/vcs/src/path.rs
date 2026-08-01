//! Paths, as a version control system spells them.

use std::path::{Path, PathBuf};

/// A path relative to the repository root, in the backend's own spelling.
///
/// Git reports forward slashes on every platform, relative to the root and
/// never to the current directory. Keeping that in one type stops a Windows
/// backslash or a `../` from leaking into a lookup key.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct RelPath(String);

impl RelPath {
    pub fn new(path: impl Into<String>) -> Self {
        Self(path.into())
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }

    /// Resolved against a repository root.
    pub fn to_absolute(&self, root: &Path) -> PathBuf {
        root.join(&self.0)
    }

    /// The final component, for display.
    pub fn file_name(&self) -> &str {
        self.0.rsplit('/').next().unwrap_or(&self.0)
    }
}

impl std::fmt::Display for RelPath {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}
