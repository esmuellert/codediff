//! What crosses between the two threads.
//!
//! Plain data in our own vocabulary, and deliberately nothing else. Neither
//! engine appears here: no `syntect` type, no parser type, nothing holding a
//! pointer from the C library underneath the matcher. That is not an accident
//! of what happens to be needed — it is the rule that keeps the seam a seam,
//! and it is checked by the compiler, since anything from those engines would
//! fail to be [`Send`].
//!
//! A request is **complete on its own**: everything needed to answer it from
//! scratch is in it. The worker may remember where it got to, but only as a
//! shortcut — throw its memory away and every answer is the same, which is
//! what makes it a cache rather than a session.

use std::sync::Arc;

use align::DiffVersion;
use file_types::File;
use syntax::Span;

/// Which content a request is about.
///
/// Compared, never interpreted. Its only job is to let a late answer for a
/// file that has since changed be told apart from a current one — nothing
/// can produce a stale answer today, but a file watcher will.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Version(pub u64);

/// Colour these lines.
pub struct SyntaxRequest {
    /// What names this content, from [`File::name`].
    ///
    /// Compared, never read. It once was `(path, side)`, which said where a
    /// version was drawn rather than which bytes it was, so the staged and
    /// the on-disk copy of one file shared a name.
    pub key: String,
    /// The path on *this* side, which is what decides the language.
    ///
    /// Beside the key rather than read out of it: a key has no path in it to
    /// find. `b87b24c…:Makefile` has no `/`, so anything looking for the last
    /// path component would take the whole string and `Makefile` would stop
    /// being a language.
    pub path: String,
    pub version: Version,
    /// Shared, not copied. A request per scroll would otherwise copy a whole
    /// file each time.
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
