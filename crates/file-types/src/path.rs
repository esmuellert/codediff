//! Where a file lives.

use std::path::{Path, PathBuf};

/// A file's location, in both spellings that anything needs.
///
/// Git reports forward slashes on every platform, relative to the repository
/// root and never to the current directory. The filesystem wants an absolute
/// path. Both are carried because a path that leaves the layer holding the
/// root cannot derive the other form later — and passing the root alongside
/// as a second value is a mismatch waiting to happen.
///
/// `codediff.nvim` reached the same conclusion and for the same reason: one
/// type carrying both, built in a single place that "knows the absolute ⇄
/// relative mapping" (`lua/codediff/core/path.lua:96`). What is *not* copied
/// from it is the empty string: there, `relative = ""` means no file, or the
/// file is the root, or the file is outside the root, and nothing can tell
/// which. Here the fields are private, there is one constructor, and absence
/// is [`Option<RepoPath>`] at the level above.
///
/// Equality covers both forms, so the same relative path under two different
/// roots is two different files — which is what it is.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct RepoPath {
    relative: String,
    absolute: PathBuf,
}

impl RepoPath {
    /// Builds a path from git's spelling and the repository root.
    ///
    /// The one place that knows the mapping, which is what keeps the two forms
    /// from disagreeing.
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

    /// The final component, for a display too narrow for the whole path.
    pub fn file_name(&self) -> &str {
        self.relative.rsplit('/').next().unwrap_or(&self.relative)
    }

    /// The repository root this path was built against.
    ///
    /// Recovered by stripping the relative tail off the absolute form, so it
    /// costs no IO and cannot disagree with either. That is the reason both
    /// forms are carried: a path that has left the layer holding the root can
    /// still resolve another one beside it.
    pub fn root(&self) -> &Path {
        let mut root = self.absolute.as_path();
        // One `pop` per component of the relative form. `Path::ancestors`
        // would be shorter but would not check that the two forms agree.
        for _ in self.relative.split('/').filter(|part| !part.is_empty()) {
            root = root.parent().unwrap_or(Path::new(""));
        }
        root
    }

    /// Everything before the final component, empty at the root.
    ///
    /// Separate from [`file_name`](Self::file_name) so a status line can style
    /// them differently and drop the directory first when the width runs out.
    /// That is the whole reason this type exists rather than a `String`.
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
        // The point of the type. A status line that is handed one string can
        // neither style these differently nor drop the directory first.
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
        // What lets a `RepoPath` resolve a sibling without the root being
        // passed alongside it — the mismatch that arrangement invites.
        assert_eq!(at("src/main.rs").root(), Path::new("/repo"));
        assert_eq!(at("README.md").root(), Path::new("/repo"));
        assert_eq!(at("a/b/c/d.rs").root(), Path::new("/repo"));
    }

    #[test]
    fn identity_follows_the_relative_form() {
        // Two roots, one file. Ordering and hashing must not depend on how the
        // root was spelled, or the same file would be two keys.
        let here = RepoPath::new("src/main.rs", Path::new("/repo"));
        let there = RepoPath::new("src/main.rs", Path::new("/elsewhere"));
        assert_ne!(here, there, "the absolute form is part of the value");
        assert_eq!(here.as_str(), there.as_str());
    }
}
