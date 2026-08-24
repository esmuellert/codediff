//! Buffer types: side-by-side, inline, single file, explorer.
//!
//! [`Buffer`] holds what every buffer has (rows, changed blocks, navigation).
//! [`BufferType`] holds only what differs between kinds.
//!
//! Side-by-side and inline are separate variants because they produce
//! different row counts from the same diff.

#[allow(clippy::module_inception)]
mod buffer;
pub mod colour;
pub mod explorer;
mod inline;
mod side_by_side;
mod single_file;

pub use buffer::{Buffer, Direction};
pub use explorer::Explorer;
pub use inline::Inline;
pub use side_by_side::SideBySide;
pub use single_file::SingleFile;

use align::Alignment;
use file_types::DiffType;
use file_types::File;

/// Which type of buffer, and what only that type holds.
#[derive(Debug)]
pub enum BufferType {
    /// Two versions, in two columns.
    SideBySide(SideBySide),
    /// Two versions, one per view line.
    Inline(Inline),
    /// One version of a file, with nothing to compare it against.
    SingleFile(SingleFile),
    /// The list of changed files, rather than any one of them.
    Explorer(Explorer),
}

impl BufferType {
    /// Which of the three ways this shows a file, or `None` for the list.
    ///
    /// A second answer, not an absent one: the explorer is a list *of* files
    /// and so is none of them. Every other kind has a [`DiffType`], including
    /// the single file — which used to be a `None` here, and was one of four
    /// places that spelled the same fork as an absence. See D60.
    pub fn diff_type(&self) -> Option<DiffType> {
        match self {
            BufferType::SideBySide(_) => Some(DiffType::SideBySide),
            BufferType::Inline(_) => Some(DiffType::Inline),
            BufferType::SingleFile(_) => Some(DiffType::Single),
            BufferType::Explorer(_) => None,
        }
    }

    /// The pairing to draw from, for the types that have one.
    pub fn alignment(&self) -> Option<&Alignment> {
        match self {
            BufferType::SideBySide(d) => Some(d.alignment()),
            BufferType::Inline(d) => Some(d.alignment()),
            BufferType::SingleFile(_) | BufferType::Explorer(_) => None,
        }
    }

    /// Which file this shows, or `None` for a buffer that is not one file.
    ///
    /// An `Option` because the explorer is a list *of* files and so is none of
    /// them. Returning the first, or an invented empty one, would put a name
    /// in the status line that nothing on screen corresponds to.
    pub fn file(&self) -> Option<&File> {
        match self {
            BufferType::SideBySide(d) => Some(d.file()),
            BufferType::Inline(d) => Some(d.file()),
            BufferType::SingleFile(f) => Some(f.file()),
            BufferType::Explorer(_) => None,
        }
    }

    /// How many lines the shown version has, for a kind with no pairing to
    /// count view lines from.
    fn lines(&self) -> u32 {
        match self {
            BufferType::SingleFile(f) => f.lines(),
            BufferType::Explorer(e) => e.view_lines(),
            _ => 0,
        }
    }
}
