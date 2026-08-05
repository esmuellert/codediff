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

use crate::syntax::{Key, Spans, Store, Syntax, SyntaxRequest, Version, key_of};

/// One file's two versions, paired up.
#[derive(Debug)]
pub struct Diff {
    file: File,
    alignment: Alignment,
    /// Which store entries hold this file's colours, one side each.
    ///
    /// Keys rather than spans: the colours belong to the store, which is
    /// what lets a file keep them when the buffer showing it closes. `None`
    /// is a side the file does not exist on, which has no text and so no
    /// language.
    original: Option<Key>,
    modified: Option<Key>,
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
            original: key_of(&file, DiffVersion::Original),
            modified: key_of(&file, DiffVersion::Modified),
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
            let Some(key) = self.key(side).cloned() else {
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
                version,
                text: self.alignment.text(side),
                have,
                want,
            });
        }
    }

    fn key(&self, side: DiffVersion) -> Option<&Key> {
        match side {
            DiffVersion::Original => self.original.as_ref(),
            DiffVersion::Modified => self.modified.as_ref(),
        }
    }

    /// The colouring of both versions, for a frame.
    ///
    /// Borrowed from the store, so this is a lookup rather than anything the
    /// diff holds. A side with nothing yet draws plainly.
    pub fn spans<'a>(&self, store: &'a Store) -> Spans<'a> {
        Spans::Both {
            original: self.original.as_ref().and_then(|key| store.get(key)),
            modified: self.modified.as_ref().and_then(|key| store.get(key)),
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
