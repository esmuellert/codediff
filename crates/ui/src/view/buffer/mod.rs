//! What a pane can show.
//!
//! A buffer is **a sequence of rows you can scroll through**, and that is the
//! whole definition. DiffVersion-by-side and inline are therefore *different buffers*
//! over the same diff, not one buffer with a flag: they emit different row
//! sequences, so "row 40" would otherwise mean different things depending on a
//! field stored somewhere else.
//!
//! Two things deliberately live elsewhere:
//!
//! - **Position.** `top`, `cursor` and `left` belong to the [`Viewport`] on
//!   the pane, because two panes showing one buffer need independent
//!   positions. A buffer is *lent* a viewport when it acts.
//! - **Size.** A buffer never knows its own width; the layout tells it.
//!
//! An enum rather than a trait: the kinds are a closed set, so an exhaustive
//! `match` means adding one breaks the build until it is handled everywhere —
//! the same property that stops the keymap growing dead commands. Zellij's
//! `Box<dyn Pane>` is the counter-example; it forced `Rc<RefCell<_>>`
//! throughout because two panes cannot be borrowed mutably through trait
//! objects.
//!
//! [`Viewport`]: crate::view::Viewport

mod side_by_side;
mod single_file;

pub use side_by_side::{Direction, SideBySide};
pub use single_file::SingleFile;

use crate::input::{BufferAction, Context};
use file_types::File;

use crate::view::Viewport;

/// Something a pane can show.
#[derive(Debug)]
pub enum Buffer {
    /// Two versions, in two columns.
    SideBySide(SideBySide),
    /// One version of a file, with nothing to compare it against.
    SingleFile(SingleFile),
}

impl Buffer {
    /// How many rows this buffer has.
    ///
    /// The only thing a generic motion needs, which is why every motion works
    /// on every buffer kind without any of them implementing one.
    pub fn rows(&self) -> u32 {
        match self {
            Buffer::SideBySide(d) => d.rows(),
            Buffer::SingleFile(f) => f.rows(),
        }
    }

    /// Which keymap is live while this buffer has focus.
    ///
    /// One context per kind by default. Sharing is possible — several
    /// list-like buffers could use one — but it has to be chosen, because a
    /// shared context means a key bound for one kind is delivered to the
    /// other, and the receiver has to ignore it.
    pub fn context(&self) -> Context {
        match self {
            Buffer::SideBySide(_) => Context::SideBySide,
            Buffer::SingleFile(_) => Context::SingleFile,
        }
    }

    /// Which file this buffer is showing.
    ///
    /// Structured, not a formatted name: the status line styles the directory
    /// differently from the file name and drops it first when the width runs
    /// out, which a single string could not support. See D28.
    pub fn file(&self) -> &File {
        match self {
            Buffer::SideBySide(d) => d.file(),
            Buffer::SingleFile(f) => f.file(),
        }
    }

    /// Applies a command aimed at what the reader is looking at.
    ///
    /// The viewport is lent, not owned: the buffer moves a position that
    /// belongs to the pane. That is what lets a motion be generic while a
    /// buffer kind can still specialise one.
    pub fn act(&mut self, action: BufferAction, count: u32, view: &mut Viewport) {
        match self {
            Buffer::SideBySide(d) => d.act(action, count, view),
            Buffer::SingleFile(f) => f.act(action, count, view),
        }
    }
}
