//! What every buffer has, whatever it is showing.
//!
//! A buffer is **a sequence of view lines you can scroll through**, and that is
//! the whole definition. Everything that follows from it lives here — how many
//! there are, which of them changed, and where change navigation last got to — so
//! that it is written once and cannot come to mean different things in
//! different layouts.
//!
//! What differs between the kinds lives in [`BufferType`] and its structs.
//! Rust has no inheritance, so the split is composition plus an enum rather
//! than a base class: the enum is only there because the language needs the
//! alternatives named.
//!
//! Two things deliberately live elsewhere:
//!
//! - **Position.** `top`, `cursor` and `left` belong to the [`Viewport`] on
//!   the pane, because two panes showing one buffer need independent
//!   positions. A buffer is *lent* a viewport when it acts.
//! - **Size.** A buffer never knows its own width; the layout tells it.

use std::ops::Range;

use align::{Alignment, DiffLayout};
use file_types::File;

use super::{BufferType, Explorer, Inline, SideBySide, SingleFile};
use crate::diff::Diff;
use crate::input::{BufferAction, KeymapType};
use crate::syntax::{Store, Syntax, Version};
use crate::view::Viewport;

/// A sequence of view lines you can scroll through.
#[derive(Debug)]
pub struct Buffer {
    /// The height of the document, in view lines rather than file lines.
    ///
    /// Not a fact about what is being shown but about **how**: a change is as
    /// tall as its taller side in two columns and as tall as both sides
    /// together inline. Cached here, beside the type that decided it, so the
    /// two are set at the same moment and cannot describe different layouts.
    view_lines: u32,
    /// Runs of adjacent changed view lines, in this buffer's own view layout.
    ///
    /// Navigation and the status line both read this, so they cannot disagree
    /// about what counts as a change — a disagreement that was a real bug.
    /// Empty when there is nothing to compare against.
    blocks: Vec<Range<u32>>,
    /// Which way the last change-navigation key went when there was nowhere
    /// left to go.
    ///
    /// Kept so the status line can answer the keypress. Without it `]c` at the
    /// last change does nothing and says nothing, which reads as a broken key
    /// rather than as the end of the file. Cleared by the next key, which is
    /// how vim's echo area behaves — and the reason this needs no clock, which
    /// `ui` is forbidden from having.
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
    /// A diff, laid out the given way.
    pub fn diff(diff: Diff, layout: DiffLayout) -> Self {
        Self::of(match layout {
            DiffLayout::SideBySide => BufferType::SideBySide(SideBySide::new(diff)),
            DiffLayout::Inline => BufferType::Inline(Inline::new(diff)),
        })
    }

    /// One version of a file, with nothing to compare it against.
    pub fn single_file(file: File, lines: &[&str]) -> Self {
        Self::of(BufferType::SingleFile(SingleFile::new(file, lines)))
    }

    /// The list of changed files.
    pub fn explorer(groups: explorer::Groups) -> Self {
        Self::of(BufferType::Explorer(Explorer::new(groups)))
    }

    /// The one place `view_lines` and `blocks` are computed, so neither can be
    /// set without the other or without the layout they were derived from.
    fn of(buffer_type: BufferType) -> Self {
        let (view_lines, blocks) = counts(&buffer_type);
        Self {
            view_lines,
            blocks,
            exhausted: None,
            buffer_type,
        }
    }

    /// Reads the same diff the other way round.
    ///
    /// Consumes and rebuilds rather than mutating, because the view-line count
    /// and the changed blocks both follow from the layout and neither is
    /// meaningful against the other one. The divider does not come along: it
    /// belongs to [`SideBySide`] and inline has no columns to divide, so there
    /// is nowhere for it to wait. A buffer with nothing to compare against has
    /// only one way to be read and is returned as it was.
    ///
    /// The reader's place is *not* carried here: a view-line number means
    /// nothing in the other layout. [`View::toggle_layout`] translates it
    /// through the file line, which does mean the same in both.
    ///
    /// [`View::toggle_layout`]: crate::view::View::toggle_layout
    pub fn flipped(self) -> Self {
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

    /// Asks for everything up to `want`, whatever this buffer is showing.
    ///
    /// Returns at once. The colours arrive over the following frames, and the
    /// buffer draws plainly until they do — which is the whole point of
    /// colouring having a thread: nothing here can be made to wait.
    ///
    /// Sends nothing when the store already has enough, which after the first
    /// screen is the ordinary case.
    pub fn request(&mut self, syntax: &mut Syntax, store: &mut Store, version: Version, want: u32) {
        match &mut self.buffer_type {
            BufferType::SideBySide(d) => d.request(syntax, store, version, want),
            BufferType::Inline(d) => d.request(syntax, store, version, want),
            BufferType::SingleFile(f) => f.request(syntax, store, version, want),
            // A list of file names is not code, so there is no language to
            // colour it by. Its colours come from the theme alone.
            BufferType::Explorer(_) => {}
        }
    }

    /// How many view lines this buffer has.
    ///
    /// The only thing a generic motion needs, which is why every motion works
    /// on every buffer kind without any of them implementing one.
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

    /// How this diff is laid out, if it is a diff at all.
    pub fn layout(&self) -> Option<DiffLayout> {
        self.buffer_type.layout()
    }

    pub fn alignment(&self) -> Option<&Alignment> {
        self.buffer_type.alignment()
    }

    pub fn hit_timeout(&self) -> bool {
        self.buffer_type
            .alignment()
            .is_some_and(Alignment::hit_timeout)
    }

    /// Which file this buffer is showing.
    ///
    /// Structured, not a formatted name: the status line styles the directory
    /// differently from the file name and drops it first when the width runs
    /// out, which a single string could not support. See D28.
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

    /// Acts on the selected row of a list, if this buffer is one.
    ///
    /// Returns whether the buffer dealt with it. `false` means the row is a
    /// file, which only something above this crate can open — so the answer
    /// is what decides whether a task leaves at all.
    pub fn select(&mut self, cursor: u32) -> bool {
        let BufferType::Explorer(explorer) = &mut self.buffer_type else {
            return false;
        };
        if !explorer.toggle(cursor) {
            return false;
        }
        self.recount();
        true
    }

    /// Where the reader should start.
    ///
    /// Zero for anything showing text, because the top of a file is where a
    /// file begins. A list starts on its first *file*: row zero is a heading,
    /// which can be folded but not opened, so starting there would mean the
    /// first key press did nothing.
    pub fn start_row(&self) -> u32 {
        match &self.buffer_type {
            BufferType::Explorer(explorer) => explorer.first_file(),
            _ => 0,
        }
    }

    /// Rebuilds the row count after the model has changed under it.
    ///
    /// A fold changes how many rows there are, and `view_lines` is what every
    /// motion and every scroll is clamped against. Called by whoever changed
    /// the model, in the same breath, so the two cannot be left disagreeing.
    pub fn recount(&mut self) {
        let (view_lines, blocks) = counts(&self.buffer_type);
        self.view_lines = view_lines;
        self.blocks = blocks;
    }

    /// Which keymap is live while this buffer has focus.
    ///
    /// One keymap_type per kind, and per layout: a diff read inline has no
    /// second column, so the keys that move the divider are not bound there.
    pub fn keymap_type(&self) -> KeymapType {
        match &self.buffer_type {
            BufferType::SideBySide(_) => KeymapType::Diff(DiffLayout::SideBySide),
            BufferType::Inline(_) => KeymapType::Diff(DiffLayout::Inline),
            BufferType::SingleFile(_) => KeymapType::SingleFile,
            BufferType::Explorer(_) => KeymapType::Explorer,
        }
    }

    /// Applies a command aimed at what the reader is looking at.
    ///
    /// The viewport is lent, not owned: the buffer moves a position that
    /// belongs to the pane. That is what lets a motion be generic while a
    /// buffer kind can still specialise one.
    pub fn act(&mut self, action: BufferAction, count: u32, view: &mut Viewport) {
        // Any key answers the previous one, so the note lasts exactly until
        // the reader does something else.
        self.exhausted = None;
        match action {
            // Generic arithmetic over a line count, which this buffer supplies.
            // Nothing here is diff-specific.
            BufferAction::Motion(motion) => view.motion(motion, count, self.view_lines),
            BufferAction::NextChange => self.step(Direction::Next, count, view),
            BufferAction::PrevChange => self.step(Direction::Previous, count, view),
            // Only two columns have a divider between them. Not bound in the
            // other contexts, so it cannot arrive there — but the match is
            // exhaustive, which is what stops a new action being forgotten.
            BufferAction::WidenOriginal | BufferAction::NarrowOriginal => {
                if let BufferType::SideBySide(data) = &mut self.buffer_type {
                    data.drag(action, count);
                }
            }
            // Both reshape the list, so both change how many rows there are.
            // `recount` keeps the motions clamped to the new number, and the
            // reshape keeps the reader on the file they were reading — landing
            // them on row zero, which is a heading nothing can open, was the
            // first version of this and it was wrong.
            BufferAction::ToggleViewMode | BufferAction::ToggleStats => {
                if let BufferType::Explorer(explorer) = &mut self.buffer_type {
                    let landing = explorer.reshape(view.cursor(), |model| match action {
                        BufferAction::ToggleViewMode => model.toggle_mode(),
                        _ => model.toggle_stats(),
                    });
                    self.recount();
                    view.jump(landing, self.view_lines);
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
        let moved = view.step(count, self.view_lines, |from| match direction {
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
    match (buffer_type.alignment(), buffer_type.layout()) {
        (Some(alignment), Some(layout)) => {
            (alignment.view_line_count(layout), alignment.blocks(layout))
        }
        // Nothing to compare against: one view line per file line, and no
        // changes, because nothing here changed *relative to* anything.
        _ => (buffer_type.lines(), Vec::new()),
    }
}
