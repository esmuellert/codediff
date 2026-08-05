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

/// Which file, and which of its two sides.
///
/// **The path is the identity, and also the language.** One string doing both
/// jobs is not a shortcut: the language is decided from the path on *this*
/// side, because a `.py` renamed to a `.rs` is Python on the left and Rust on
/// the right, and showing either as the other would be a lie the reader can
/// see.
///
/// This is enough while a review is one comparison — worktree against `HEAD` —
/// because then a path has exactly one original and one modified. Comparing
/// arbitrary revisions will need git's object id instead, which is better
/// still: an id *is* the content hash, so two files sharing one could share a
/// cache entry.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct Key {
    pub path: String,
    pub side: DiffVersion,
}

impl Key {
    pub fn new(path: impl Into<String>, side: DiffVersion) -> Self {
        Self {
            path: path.into(),
            side,
        }
    }
}

/// Colour these lines.
pub struct SyntaxRequest {
    pub key: Key,
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
    pub want: u32,
}

/// Some of a file, coloured.
///
/// A file arrives in pieces, oldest first, because a reader should not watch
/// plain text for the sixteen seconds a three-hundred-thousand-line file takes
/// with the slower engine. `from` is the line the piece starts at, so the
/// asker appends without needing to know how many pieces there will be.
pub struct SyntaxResponse {
    pub key: Key,
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

/// The key for one side of a file, if that side exists.
pub fn key_of(file: &File, version: DiffVersion) -> Option<Key> {
    path_of(file, version).map(|path| Key::new(path, version))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_two_sides_of_one_file_are_different_keys() {
        // Same path, different content: an unchanged path still has two
        // versions, and colouring one is not colouring the other.
        let left = Key::new("src/main.rs", DiffVersion::Original);
        let right = Key::new("src/main.rs", DiffVersion::Modified);
        assert_ne!(left, right);
    }

    #[test]
    fn a_renamed_file_keeps_each_side_under_its_own_name() {
        // Which is what lets the language be read off the key.
        let left = Key::new("old.py", DiffVersion::Original);
        let right = Key::new("new.rs", DiffVersion::Modified);
        assert_ne!(left, right);
        assert!(left.path.ends_with(".py"));
        assert!(right.path.ends_with(".rs"));
    }
}
