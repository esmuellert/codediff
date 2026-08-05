//! What the pipeline hands over.
//!
//! ---
//!
//! One file's two versions and the pairing between them, and nothing about how
//! any of it is shown. Defined here, by the consumer, rather than by whatever
//! assembles it — that is the direction that keeps the crate graph acyclic,
//! since the composition root already depends on `ui` and the reverse
//! would be a cycle.
//!
//! It is **data, not a projection.** How many rows there are is not a property
//! of a diff: an [`align::ViewLine`] is a *pair*, so a row count is already an
//! answer to "how would this look side by side". A buffer decides that, and
//! caches the answer beside the decision. See [`SideBySide`].
//!
//! [`SideBySide`]: crate::view::buffer::SideBySide

use align::{Alignment, DiffVersion};
use file_types::File;

use crate::syntax::{Spans, Store, Syntax, SyntaxRequest, Version, path_of};

/// One file's two versions, paired up.
#[derive(Debug)]
pub struct Diff {
    file: File,
    alignment: Alignment,
    /// Which content those keys are for, so a late answer for a file that
    /// has since changed can be told apart and dropped.
    version: Version,
}

impl Diff {
    /// A diff that has not been coloured and never will be.
    ///
    /// What the pipeline builds. Colouring starts when the interface asks for
    /// it, which is the composition root's business rather than the
    /// pipeline's — `ui` owns the thread because `ui` owns the loop that
    /// collects from it.
    pub fn new(file: File, alignment: Alignment) -> Self {
        Self {
            file,
            alignment,
            version: Version(0),
        }
    }

    /// Asks for everything up to `want`, on both sides.
    ///
    /// Sends nothing for a side the store already holds enough of, which is
    /// the ordinary case after the first screen and the whole reason the
    /// store is on this side of the thread.
    pub fn request(&mut self, syntax: &mut Syntax, store: &mut Store, version: Version, want: u32) {
        self.version = version;
        for side in [DiffVersion::Original, DiffVersion::Modified] {
            let (Some(key), Some(path)) = (self.key(side), path_of(&self.file, side)) else {
                continue;
            };
            let lines = self.alignment.lines(side).len() as u32;
            let want = want.min(lines.saturating_sub(1));
            if lines == 0 || syntax.busy(&key) {
                continue;
            }
            store.want(&key, version);
            let have = store.have(&key);
            if have > want {
                continue;
            }
            syntax.send(SyntaxRequest {
                key,
                path,
                version,
                text: self.alignment.text(side),
                have,
                want,
            });
        }
    }

    /// What names one side's content, if the file is on that side.
    ///
    /// Asked of the file each time rather than held, because it is derived —
    /// storing it beside the file is how a copy comes to disagree with what
    /// it was copied from.
    fn key(&self, side: DiffVersion) -> Option<String> {
        self.file.name(side)
    }

    /// The colouring of both versions, for a frame.
    ///
    /// Borrowed from the store, so this is a lookup rather than anything the
    /// diff holds. A side with nothing yet draws plainly.
    pub fn spans<'a>(&self, store: &'a Store) -> Spans<'a> {
        Spans::Both {
            original: self
                .key(DiffVersion::Original)
                .and_then(|key| store.get(&key)),
            modified: self
                .key(DiffVersion::Modified)
                .and_then(|key| store.get(&key)),
        }
    }

    /// What to draw from.
    ///
    /// A borrow of something already built, not a construction: the pipeline
    /// paired the lines up once, when the file was opened.
    pub fn alignment(&self) -> &Alignment {
        &self.alignment
    }

    /// Which file this is — structured, never a formatted string.
    ///
    /// A `label: String` here used to fuse the path, the previous path and the
    /// added/deleted note, after which the status line could neither style nor
    /// shorten them separately. See D28.
    pub fn file(&self) -> &File {
        &self.file
    }

    /// The engine gave up early, so the pairing is coarser than the files
    /// warrant. The reader has to be told.
    pub fn hit_timeout(&self) -> bool {
        self.alignment.hit_timeout()
    }
}

#[cfg(test)]
mod tests {
    use file_types::{Oid, RepoPath, Rev, Revs};

    use super::*;
    use crate::syntax::Syntax;

    fn at(path: &str) -> RepoPath {
        RepoPath::new(path, std::path::Path::new("/repo"))
    }

    /// A file against itself. No engine is run — `ui` may not name one — and
    /// none is needed: what these check is which entries a request makes, not
    /// what the pairing says.
    fn alignment(lines: &[&str]) -> Alignment {
        Alignment::new(
            diff_types::LinesDiff {
                changes: Vec::new(),
                moves: Vec::new(),
                hit_timeout: false,
            },
            lines,
            lines,
        )
    }

    /// One diff of one path, read against `HEAD`, with the after side named.
    fn diff(after: Rev) -> Diff {
        let revs = Revs::new(Rev::Commit(Oid::new("b87b24c")), after);
        Diff::new(
            File::unchanged_path(at("src/main.rs"), revs),
            alignment(&["fn main() {}"]),
        )
    }

    #[test]
    fn the_staged_and_the_working_copy_of_one_path_do_not_share_a_cache_entry() {
        // The old key said which column a version was drawn in, so both of
        // these were one name over two different sets of bytes.
        let mut syntax = Syntax::start();
        let mut store = Store::new();

        for after in [Rev::Worktree, Rev::Index] {
            diff(after).request(&mut syntax, &mut store, Version(1), 0);
        }

        assert_eq!(
            store.entries(),
            3,
            "one entry for the shared before side, and one for each after side"
        );
    }

    #[test]
    fn two_files_read_against_one_commit_share_that_side() {
        // The other half, and free: a commit is named by its id, so the before
        // side of every file in a review that happens to be the same blob is
        // the same entry.
        let mut syntax = Syntax::start();
        let mut store = Store::new();
        diff(Rev::Worktree).request(&mut syntax, &mut store, Version(1), 0);
        let before = store.entries();
        diff(Rev::Worktree).request(&mut syntax, &mut store, Version(1), 0);
        assert_eq!(store.entries(), before, "asking twice made no new entry");
    }
}
