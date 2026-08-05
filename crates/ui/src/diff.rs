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

use crate::paint::{Colours, Job, Painter, Spans, Version, path_of};

/// One file's two versions, paired up.
#[derive(Debug)]
pub struct Diff {
    file: File,
    alignment: Alignment,
    /// The colours the painter has sent back so far, one side each.
    ///
    /// Spans, not a highlighter: colouring happens on another thread, and what
    /// crosses back is plain data. See [`crate::paint`].
    original: Colours,
    modified: Colours,
    /// Which request these answer, so a late one for a file that has since
    /// changed can be told apart and dropped.
    version: Version,
}

impl Diff {
    /// A diff that has not been coloured and never will be.
    ///
    /// What the pipeline builds. Colouring starts when a [`Painter`] is handed
    /// over, which is the composition root's business rather than the
    /// pipeline's — `ui` owns the thread because `ui` owns the loop that
    /// collects from it.
    pub fn new(file: File, alignment: Alignment) -> Self {
        Self {
            file,
            alignment,
            original: Colours::default(),
            modified: Colours::default(),
            version: Version(0),
        }
    }

    /// Asks the painter for both versions.
    ///
    /// Returns at once; the colours arrive over the following frames.
    pub fn start_painting(&mut self, painter: &Painter, version: Version) {
        self.version = version;
        for (n, side) in [DiffVersion::Original, DiffVersion::Modified]
            .into_iter()
            .enumerate()
        {
            let Some(path) = path_of(&self.file, side) else {
                continue;
            };
            painter.paint(Job {
                // Two requests per diff, so the two sides need telling apart.
                version: Version(version.0 * 2 + n as u64),
                path,
                lines: self.alignment.lines(side).to_vec(),
            });
        }
    }

    /// Installs a piece the painter finished, if it is still wanted.
    ///
    /// Returns whether anything changed, so the caller knows to redraw.
    pub fn install(&mut self, painted: crate::paint::Painted) -> bool {
        let side = match painted.version.0.checked_sub(self.version.0 * 2) {
            Some(0) => &mut self.original,
            Some(1) => &mut self.modified,
            // For an older version of this file. Nothing can produce one yet,
            // but a file watcher will.
            _ => return false,
        };
        side.install(painted);
        true
    }

    /// Whether either side is still waiting for colours.
    pub fn painting(&self) -> bool {
        [DiffVersion::Original, DiffVersion::Modified]
            .into_iter()
            .zip([&self.original, &self.modified])
            .any(|(side, colours)| {
                // A side that does not exist has no lines and nothing to wait
                // for, which the comparison already says.
                colours.lines() < self.alignment.lines(side).len() as u32
            })
    }

    /// The colouring of both versions, for a frame.
    pub fn spans(&self) -> Spans<'_> {
        Spans::Both {
            original: &self.original,
            modified: &self.modified,
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
