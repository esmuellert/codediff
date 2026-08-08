//! What crosses between the two threads.
//!
//! Plain data only — no engine types. The compiler enforces this via `Send`.

use std::sync::Arc;

use align::DiffVersion;
use file_types::File;
use syntax::Span;

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
    /// How many lines from the top the asker already holds.
    ///
    /// Not a courtesy: it is how the worker knows whether its memory of this
    /// file is still worth anything. If the asker has thrown its colours away
    /// — which eviction does — then a bookmark half way down the file answers
    /// a question nobody asked, and reading must start again from the top.
    pub have: u32,
    /// The last line the asker needs, counted from 0.
    pub last: u32,
}

/// Some of a file, coloured.
///
/// A file arrives in pieces, oldest first, because a reader should not watch
/// plain text for the sixteen seconds a three-hundred-thousand-line file takes
/// with the slower engine. `from` is the line the piece starts at, so the
/// asker appends without needing to know how many pieces there will be.
pub struct SyntaxResponse {
    /// The request's key, handed back untouched.
    pub key: String,
    pub version: Version,
    pub from: u32,
    pub spans: Vec<Vec<Span>>,
    /// Whether another piece is coming for the request this answers.
    ///
    /// What tells the asker a request is finished, so it can send the next
    /// one. Without it a fast scroll could only guess, and guessing wrong
    /// either floods the queue or stalls the file.
    pub more: bool,
}

/// The path a version of a file is known by, if it exists on that side.
///
/// A file added or deleted exists on one side only, and the side it does not
/// exist on has no text and therefore no language.
pub fn path_of(file: &File, version: DiffVersion) -> Option<String> {
    file.on(version).map(|path| path.as_str().to_owned())
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
        // Same path, different content: an unchanged path still has two
        // versions, and colouring one is not colouring the other.
        let file = File::unchanged_path(at("src/main.rs"), revs());
        assert_ne!(
            file.name(DiffVersion::Original),
            file.name(DiffVersion::Modified)
        );
    }

    #[test]
    fn a_renamed_file_keeps_each_side_under_its_own_name() {
        // Which is what lets the language be read off the path.
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
