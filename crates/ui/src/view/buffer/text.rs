//! One file, with nothing to compare it against.
//!
//! An added or deleted file, or simply a file being looked at. There is no
//! second side, so there is no diff, no highlighting and no second column —
//! not as a special case of a diff, but because this is a different kind of
//! buffer. VSCode reached the same place: it stopped opening a diff editor for
//! added, untracked and deleted files, because an empty left-hand side "did
//! not provide much value". See D23.

use crate::input::BufferAction;
use crate::view::Viewport;

/// One file's text.
#[derive(Debug)]
pub struct Text {
    label: String,
    lines: Vec<String>,
}

impl Text {
    /// Copies the lines in, as [`Alignment::new`] does, so a caller holding
    /// borrowed lines need not convert them first.
    ///
    /// [`Alignment::new`]: align::Alignment::new
    pub fn new(label: String, lines: &[&str]) -> Self {
        Self {
            label,
            lines: lines.iter().map(|line| (*line).to_owned()).collect(),
        }
    }

    pub fn label(&self) -> &str {
        &self.label
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
