//! Paths inside a repository.

use std::path::{Path, PathBuf};

/// A repository-relative path and its absolute filesystem path.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct RepoPath {
    relative: String,
    absolute: PathBuf,
}

impl RepoPath {
    /// Builds a path from Git's spelling and the repository root.
    pub fn new(relative: impl Into<String>, root: &Path) -> Self {
        let relative = relative.into();
        Self {
            absolute: root.join(&relative),
            relative,
        }
    }

    /// As git spells it: relative to the root, forward slashes.
    ///
    /// What goes to git, and what is shown on screen.
    pub fn as_str(&self) -> &str {
        &self.relative
    }

    /// As the filesystem wants it.
    pub fn as_path(&self) -> &Path {
        &self.absolute
    }

    /// Returns the final path component.
    pub fn file_name(&self) -> &str {
        self.relative.rsplit('/').next().unwrap_or(&self.relative)
    }

    /// The repository root this path was built against.
    ///
    /// Returns the repository root.
    pub fn root(&self) -> &Path {
        let mut root = self.absolute.as_path();
        // Remove one component for each relative path component.
        for _ in self.relative.split('/').filter(|part| !part.is_empty()) {
            root = root.parent().unwrap_or(Path::new(""));
        }
        root
    }

    /// Everything before the final component, empty at the root.
    ///
    /// Separate from [`file_name`](Self::file_name) so a status line can style
    /// them differently and drop the directory first when the width runs out.
    pub fn directory(&self) -> &str {
        match self.relative.rfind('/') {
            Some(at) => &self.relative[..at],
            None => "",
        }
    }
}

impl std::fmt::Display for RepoPath {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.relative)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn at(relative: &str) -> RepoPath {
        RepoPath::new(relative, Path::new("/repo"))
    }

    #[test]
    fn both_forms_come_from_one_constructor() {
        let path = at("src/main.rs");
        assert_eq!(path.as_str(), "src/main.rs");
        assert_eq!(path.as_path(), Path::new("/repo/src/main.rs"));
    }

    #[test]
    fn the_name_and_the_directory_are_separately_available() {
        let path = at("crates/ui/src/app.rs");
        assert_eq!(path.file_name(), "app.rs");
        assert_eq!(path.directory(), "crates/ui/src");
    }

    #[test]
    fn a_file_at_the_root_has_no_directory() {
        let path = at("README.md");
        assert_eq!(path.file_name(), "README.md");
        assert_eq!(path.directory(), "");
    }

    #[test]
    fn the_root_comes_back_out() {
        assert_eq!(at("src/main.rs").root(), Path::new("/repo"));
        assert_eq!(at("README.md").root(), Path::new("/repo"));
        assert_eq!(at("a/b/c/d.rs").root(), Path::new("/repo"));
    }

    #[test]
    fn identity_follows_the_relative_form() {
        // The absolute root is part of the path identity.
        let here = RepoPath::new("src/main.rs", Path::new("/repo"));
        let there = RepoPath::new("src/main.rs", Path::new("/elsewhere"));
        assert_ne!(here, there, "the absolute form is part of the value");
        assert_eq!(here.as_str(), there.as_str());
    }
}
