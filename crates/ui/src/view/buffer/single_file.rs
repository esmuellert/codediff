//! One version of a file, shown alone.
//!
//! A presentation mode, and a peer of [`SideBySide`] — not a content type. It
//! is what both diff modes fall back to when a file exists on only one side:
//! there is nothing to lay out against, so neither two columns nor an
//! interleaving has anything to say.
//!
//! No second version means no alignment, no filler and no divider — one column
//! of numbered lines, in the ordinary colours. Nothing here changed *relative
//! to* anything, so nothing is highlighted; marking every line of a new file
//! green says nothing the word "added" does not. VSCode reached the same place
//! and stopped opening a diff editor for added, untracked and deleted files.
//! See D23.
//!
//! [`SideBySide`]: super::SideBySide

use file_types::File;

use crate::input::BufferAction;
use crate::view::Viewport;

/// One version of a file, and its lines.
#[derive(Debug)]
pub struct SingleFile {
    file: File,
    lines: Vec<String>,
}

impl SingleFile {
    /// Copies the lines in, as [`Alignment::new`] does, so a caller holding
    /// borrowed lines need not convert them first.
    ///
    /// [`Alignment::new`]: align::Alignment::new
    pub fn new(file: File, lines: &[&str]) -> Self {
        Self {
            file,
            lines: lines.iter().map(|line| (*line).to_owned()).collect(),
        }
    }

    /// Which file this is — structured, so a status line can style and shorten
    /// its parts independently.
    pub fn file(&self) -> &File {
        &self.file
    }

    pub fn rows(&self) -> u32 {
        self.lines.len() as u32
    }

    pub fn line(&self, row: u32) -> Option<&str> {
        self.lines.get(row as usize).map(String::as_str)
    }

    pub fn act(&mut self, action: BufferAction, count: u32, view: &mut Viewport) {
        match action {
            BufferAction::Motion(motion) => view.motion(motion, count, self.rows()),
            // Nothing changed relative to anything, so there are no changes to
            // step through and no second column to resize. These are not bound
            // in this buffer's context, so they cannot arrive — but the match
            // is exhaustive, which is what stops a new action being forgotten.
            BufferAction::NextChange
            | BufferAction::PrevChange
            | BufferAction::WidenOriginal
            | BufferAction::NarrowOriginal => {}
        }
    }
}
