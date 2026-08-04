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

use align::Alignment;
use file_types::File;

/// One file's two versions, paired up.
#[derive(Debug)]
pub struct Diff {
    file: File,
    alignment: Alignment,
}

impl Diff {
    pub fn new(file: File, alignment: Alignment) -> Self {
        Self { file, alignment }
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
