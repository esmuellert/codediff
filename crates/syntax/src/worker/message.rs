//! Messages exchanged with the syntax worker.

use std::sync::Arc;

use crate::Span;
use align::DiffVersion;
use file_types::File;

/// Monotonic counter for detecting stale answers.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Version(pub u64);

/// Colour these lines.
pub struct SyntaxRequest {
    /// Identity key from [`File::name`].
    pub key: String,
    /// The path on this side — decides the language.
    pub path: String,
    pub version: Version,
    /// Shared, not copied — avoids a copy per scroll.
    pub text: Arc<Vec<String>>,
    /// Lines already held by the caller.
    pub have: u32,
    /// The last line the asker needs, counted from 0.
    pub last: u32,
}

/// One ordered piece of a coloured file.
pub struct SyntaxResponse {
    /// The request's key, handed back untouched.
    pub key: String,
    pub version: Version,
    pub from: u32,
    pub spans: Vec<Vec<Span>>,
    /// Whether another piece follows.
    pub more: bool,
}

/// The path of a file version, when that side exists.
pub fn path_of(file: &File, version: DiffVersion) -> Option<String> {
    file.path_of_version(version)
        .map(|path| path.as_str().to_owned())
}

#[cfg(test)]
mod tests {
    use file_types::{Oid, RepoPath, Revs};

    use super::*;

    fn at(path: &str) -> RepoPath {
        RepoPath::new(path, std::path::Path::new("/repo"))
    }

    fn revs() -> Revs {
        Revs::worktree_against(Oid::new("b87b24c"))
    }

    #[test]
    fn the_two_sides_of_one_file_have_different_keys() {
        let file = File::unchanged_path(at("src/main.rs"), revs());
        assert_ne!(
            file.name(DiffVersion::Original),
            file.name(DiffVersion::Modified)
        );
    }

    #[test]
    fn a_renamed_file_keeps_each_side_under_its_own_name() {
        let file = File::renamed(at("old.py"), at("new.rs"), revs());
        assert!(
            path_of(&file, DiffVersion::Original)
                .unwrap()
                .ends_with(".py")
        );
        assert!(
            path_of(&file, DiffVersion::Modified)
                .unwrap()
                .ends_with(".rs")
        );
    }
}
