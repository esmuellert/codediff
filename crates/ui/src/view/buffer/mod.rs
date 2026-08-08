//! What a pane can show.
//!
//! [`Buffer`] is everything a buffer has whatever it holds — rows, changed
//! blocks, change navigation. `BufferType` below is only what differs between
//! the kinds. Rust has no inheritance, so a shared base is composition plus an
//! enum naming the alternatives; the enum exists because the language needs
//! them named, not because the kinds are more different than they are.
//!
//! Side by side and inline emit different row sequences over the same diff, so
//! "row 40" means different things in each. That is why they are separate
//! variants rather than one with a flag: the variant *is* the row layout, so
//! there is no field for the row count to fall out of step with, and both the
//! renderer and the keymap can dispatch on it without reading one.
//!
//! An enum rather than a trait: the kinds are a closed set, so an exhaustive
//! `match` means adding one breaks the build until it is handled everywhere —
//! the same property that stops the keymap growing dead commands. A trait
//! could not carry the shared fields anyway. Zellij's `Box<dyn Pane>` is the
//! counter-example; it forced `Rc<RefCell<_>>` throughout because two panes
//! cannot be borrowed mutably through trait objects.

#[allow(clippy::module_inception)]
mod buffer;
mod colour;
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

/// Which type of buffer this is, and what only that type holds.
///
/// The variant *is* the layout, so the renderer and the keymap dispatch on
/// something the compiler checks rather than on a field. Which walk each one
/// asks `align` for is [`DiffType`], defined once there — these variants
/// select it, they do not redefine what it means.
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
