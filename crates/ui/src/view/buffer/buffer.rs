//! The shared `Buffer` struct and its `BufferType` enum.
//!
//! What differs between kinds lives in [`BufferType`]. Position lives on
//! the pane's [`Viewport`] (two panes on one buffer scroll independently).

use std::ops::Range;

use align::Alignment;
use file_types::{DiffType, File};
use pipeline::file::DiffContent;

use super::{BufferType, Explorer, Inline, SideBySide, SingleFile};
use crate::input::{BufferAction, KeymapType};
use crate::syntax::{Store, Syntax, Version};
use crate::view::Viewport;

/// A sequence of view lines you can scroll through.
#[derive(Debug)]
pub struct Buffer {
    /// Height of the document in view lines (layout-dependent).
    view_lines: u32,
    /// Runs of changed view lines, for navigation and the status line.
    blocks: Vec<Range<u32>>,
    /// Set when `]c`/`[c` hit the end, cleared on the next key.
    exhausted: Option<Direction>,
    buffer_type: BufferType,
}

/// Which way a change-navigation key was pressed.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Direction {
    Next,
    Previous,
}

impl Buffer {
    /// Builds a buffer from what the pipeline read. See D23 and D60.
    pub fn diff(content: DiffContent) -> Self {
        Self::of(match content {
            DiffContent::SingleFile(single) => BufferType::SingleFile(SingleFile::new(single)),
            DiffContent::Diff(diff) => BufferType::SideBySide(SideBySide::new(diff)),
        })
    }

    /// The list of changed files.
    pub fn explorer(files: Vec<file_types::File>) -> Self {
        Self::of(BufferType::Explorer(Explorer::new(files)))
    }

    fn of(buffer_type: BufferType) -> Self {
        let (view_lines, blocks) = counts(&buffer_type);
        Self {
            view_lines,
            blocks,
            exhausted: None,
            buffer_type,
        }
    }

    /// The same buffer in the other layout. Position is not carried.
    pub fn switch_diff_layout(self) -> Self {
        Self::of(match self.buffer_type {
            BufferType::SideBySide(d) => BufferType::Inline(Inline::new(d.into_diff())),
            BufferType::Inline(d) => BufferType::SideBySide(SideBySide::new(d.into_diff())),
            // Nothing to compare against, so only one way to read it.
            other => other,
        })
    }

    pub fn buffer_type(&self) -> &BufferType {
        &self.buffer_type
    }

    pub fn buffer_type_mut(&mut self) -> &mut BufferType {
        &mut self.buffer_type
    }

    /// Asks the syntax worker to colour up to `last`. Returns immediately.
    pub fn request(&mut self, syntax: &mut Syntax, store: &mut Store, version: Version, last: u32) {
        match &mut self.buffer_type {
            BufferType::SideBySide(d) => d.request(syntax, store, version, last),
            BufferType::Inline(d) => d.request(syntax, store, version, last),
            BufferType::SingleFile(f) => f.request(syntax, store, version, last),
            // A list of file names is not code, so there is no language to
            // colour it by. Its colours come from the theme alone.
            BufferType::Explorer(_) => {}
        }
    }

    /// How many view lines this buffer has.
    pub fn view_lines(&self) -> u32 {
        self.view_lines
    }

    pub fn blocks(&self) -> &[Range<u32>] {
        &self.blocks
    }

    /// Which changed block a line falls in, if any.
    pub fn block_at(&self, view_line: u32) -> Option<usize> {
        self.blocks.iter().position(|b| b.contains(&view_line))
    }

    /// Which way the last change-navigation key went with nowhere to go.
    pub fn exhausted(&self) -> Option<Direction> {
        self.exhausted
    }

    /// Which of the three ways this shows a file, or `None` for the list.
    pub fn diff_type(&self) -> Option<DiffType> {
        self.buffer_type.diff_type()
    }

    pub fn alignment(&self) -> Option<&Alignment> {
        self.buffer_type.alignment()
    }

    pub fn hit_timeout(&self) -> bool {
        self.buffer_type
            .alignment()
            .is_some_and(Alignment::hit_timeout)
    }

    /// Which file this buffer is showing, or `None` for the explorer.
    pub fn file(&self) -> Option<&File> {
        self.buffer_type.file()
    }

    /// The list of changed files, when that is what this is.
    pub fn as_explorer_mut(&mut self) -> Option<&mut Explorer> {
        match &mut self.buffer_type {
            BufferType::Explorer(explorer) => Some(explorer),
            _ => None,
        }
    }

    /// Toggles a directory fold on a list. Returns `true` if handled (row was
    /// a directory), `false` if the row is a file the caller should open.
    pub fn activate(&mut self, cursor: u32) -> bool {
        let BufferType::Explorer(explorer) = &mut self.buffer_type else {
            return false;
        };
        if !explorer.toggle(cursor) {
            return false;
        }
        self.update_line_count();
        true
    }

    /// Where the reader should start. A list starts on its first file (row 0
    /// is a heading that can't be opened).
    pub fn start_row(&self) -> u32 {
        match &self.buffer_type {
            BufferType::Explorer(explorer) => explorer.first_file(),
            _ => 0,
        }
    }

    /// Rebuilds the row count after a fold or mode change.
    pub fn update_line_count(&mut self) {
        let (view_lines, blocks) = counts(&self.buffer_type);
        self.view_lines = view_lines;
        self.blocks = blocks;
    }

    /// Which keymap is live while this buffer has focus.
    pub fn keymap_type(&self) -> KeymapType {
        match self.buffer_type.diff_type() {
            Some(diff_type) => KeymapType::File(diff_type),
            None => KeymapType::Explorer,
        }
    }

    /// Applies a buffer action with the given count.
    pub fn apply(&mut self, action: BufferAction, count: u32, view: &mut Viewport) {
        self.exhausted = None;
        match action {
            BufferAction::Motion(motion) => view.motion(motion, count, self.view_lines),
            BufferAction::NextChange => self.step(Direction::Next, count, view),
            BufferAction::PrevChange => self.step(Direction::Previous, count, view),
            BufferAction::Toggle => {
                if let BufferType::Explorer(explorer) = &mut self.buffer_type {
                    explorer.toggle(view.cursor());
                    self.update_line_count();
                }
            }
            BufferAction::ToggleViewMode => {
                if let BufferType::Explorer(explorer) = &mut self.buffer_type {
                    let landing =
                        explorer.reshape_around(view.cursor(), |model| model.toggle_mode());
                    self.update_line_count();
                    view.place(landing, self.view_lines);
                }
            }
        }
    }

    /// Moves to the next or previous run of changed view lines.
    ///
    /// Written once for every layout: the blocks differ between layouts,
    /// but stepping through them does not.
    fn step(&mut self, direction: Direction, count: u32, view: &mut Viewport) {
        let starts = || self.blocks.iter().map(|b| b.start);
        let moved = view.jump_to(count, self.view_lines, |from| match direction {
            Direction::Next => starts().find(|&r| r > from),
            Direction::Previous => starts().rev().find(|&r| r < from),
        });
        if !moved {
            self.exhausted = Some(direction);
        }
    }
}

/// How tall the document is and which of its view lines changed, both of which
/// follow from the layout and neither of which is stored anywhere else.
fn counts(buffer_type: &BufferType) -> (u32, Vec<Range<u32>>) {
    match (buffer_type.alignment(), buffer_type.diff_type()) {
        (Some(alignment), Some(diff_type)) => (
            alignment.view_line_count(diff_type),
            alignment.blocks(diff_type),
        ),
        // Nothing to compare against: one view line per file line, and no
        // changes, because nothing here changed *relative to* anything.
        _ => (buffer_type.lines(), Vec::new()),
    }
}
