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
use syntax::Highlighted;

use crate::highlight::{self, Spans};

/// One file's two versions, paired up.
#[derive(Debug)]
pub struct Diff {
    file: File,
    alignment: Alignment,
    /// How far each version has been coloured.
    ///
    /// One per version, beside the lines they describe, because a rename can
    /// change the language between the two sides. Read forwards only and never
    /// invalidated: a diff under review is a snapshot, so the answer for a
    /// line once found is the answer for good.
    original: Highlighted,
    modified: Highlighted,
}

impl Diff {
    pub fn new(file: File, alignment: Alignment) -> Self {
        let original = highlight::begin(
            &file,
            DiffVersion::Original,
            alignment.lines(DiffVersion::Original),
        );
        let modified = highlight::begin(
            &file,
            DiffVersion::Modified,
            alignment.lines(DiffVersion::Modified),
        );
        Self {
            file,
            alignment,
            original,
            modified,
        }
    }

    /// The colouring of both versions, for a frame.
    pub fn spans(&self) -> Spans<'_> {
        Spans::Both {
            original: &self.original,
            modified: &self.modified,
        }
    }

    /// Colours up to the given line of each version, numbered from 1.
    ///
    /// Called with the last line a frame is about to draw, so that scrolling
    /// forward pays for the gap once and scrolling back pays nothing.
    pub fn reach(&mut self, original: u32, modified: u32) {
        highlight::reach(
            &mut self.original,
            original,
            self.alignment.lines(DiffVersion::Original),
        );
        highlight::reach(
            &mut self.modified,
            modified,
            self.alignment.lines(DiffVersion::Modified),
        );
    }

    /// Whether both versions have been coloured as far as the given lines.
    pub fn caught_up(&self, original: u32, modified: u32) -> bool {
        highlight::caught_up(&self.original, original)
            && highlight::caught_up(&self.modified, modified)
    }

    /// Colours a little more of whichever version is behind, and says whether
    /// there was anything to do.
    ///
    /// What an idle moment calls, so that a long file finishes colouring while
    /// the reader is deciding what to press. One version per call, so a slice
    /// stays a slice.
    pub fn read_more(&mut self) -> bool {
        highlight::read_more(
            &mut self.original,
            self.alignment.lines(DiffVersion::Original),
        ) || highlight::read_more(
            &mut self.modified,
            self.alignment.lines(DiffVersion::Modified),
        )
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
