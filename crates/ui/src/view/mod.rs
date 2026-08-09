//! The view model: View → Tab → Pane → Buffer.
//!
//! ```text
//! view/            View     tabs, and every buffer
//! ├ tab.rs         Tab      pane layout and focus
//! ├ pane.rs        Pane     one buffer + one Viewport
//! ├ viewport.rs    Viewport top, cursor, left
//! └ buffer/        Buffer   what a pane can show
//! ```
//!
//! Buffers live in [`View`], referenced by [`BufferId`] (not by `&mut`).

pub mod buffer;
mod pane;
pub mod selection;
mod tab;
mod viewport;

pub use buffer::{Buffer, BufferType, Direction};
pub use pane::Pane;
pub use selection::{Selection, SelectionColumn};
pub use tab::{Layout, PaneId, Tab};
pub use viewport::Viewport;

use file_types::DiffType;

use crate::input::KeymapType;
use syntax::{Store, Syntax, Version};

/// An index into [`View::buffers`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct BufferId(usize);

impl BufferId {
    /// For tests, which need to name a buffer without building one.
    #[cfg(test)]
    pub(crate) fn new(index: usize) -> Self {
        Self(index)
    }
}

/// Everything the interface is showing.
#[derive(Debug)]
pub struct View {
    buffers: Vec<Buffer>,
    tabs: Vec<Tab>,
    active: usize,
    overlays: Vec<Overlay>,
    /// Whether syntax highlighting is on for the session.
    syntax: bool,
    /// Incremented when a buffer is replaced, so the colour store discards
    /// stale spans.
    version: Version,
    /// The single active mouse text selection, if any: (pane, range).
    pub selection: Option<(PaneId, Selection)>,
}

/// Uninhabited — reserved for help/prompts.
#[derive(Debug)]
pub enum Overlay {}

impl View {
    /// Opens one buffer in one pane in one tab.
    pub fn single(buffer: Buffer) -> Self {
        let (start, rows) = (buffer.start_row(), buffer.view_lines());
        let mut view = Self {
            buffers: vec![buffer],
            tabs: vec![Tab::single(BufferId(0))],
            active: 0,
            overlays: Vec::new(),
            syntax: true,
            version: Version(1),
            selection: None,
        };
        // The buffer decides where a reader starts, and the viewport is what
        // holds the answer — so the two are set together, here, rather than
        // left for the first frame to reconcile.
        view.tabs[0].focused_mut().viewport.place(start, rows);
        view
    }

    /// Whether code is being coloured by its language.
    pub fn syntax(&self) -> bool {
        self.syntax
    }

    pub fn toggle_syntax(&mut self) {
        self.syntax = !self.syntax;
    }

    /// Asks the syntax worker to colour everything visible, plus a margin.
    pub fn request(&mut self, syntax: &mut Syntax, store: &mut Store) {
        const MARGIN: u32 = 2_000;
        let version = self.version;
        let panes: Vec<PaneId> = self.tab().ids().collect();
        for id in panes {
            let (buffer, viewport) = self.pane_mut(id);
            let visible = viewport.visible(buffer.view_lines());
            buffer.request(syntax, store, version, visible.end + MARGIN);
        }
    }

    pub fn buffer(&self, id: BufferId) -> &Buffer {
        &self.buffers[id.0]
    }

    pub fn buffer_mut(&mut self, id: BufferId) -> &mut Buffer {
        &mut self.buffers[id.0]
    }

    pub fn tab(&self) -> &Tab {
        &self.tabs[self.active]
    }

    pub fn tab_mut(&mut self) -> &mut Tab {
        &mut self.tabs[self.active]
    }

    /// The pane the reader is working in.
    pub fn focused(&self) -> &Pane {
        self.tab().focused()
    }

    /// The focused buffer and viewport together (both borrows at once).
    pub fn focused_buffer(&self) -> &Buffer {
        self.buffer(self.focused().buffer)
    }

    /// The keymap that is live, which the focused buffer decides.
    pub fn keymap_type(&self) -> KeymapType {
        self.buffer(self.focused().buffer).keymap_type()
    }

    /// The pane showing `buffer`, read-only.
    pub fn pane_for(&self, buffer: BufferId) -> &Pane {
        let tab = self.tab();
        let id = tab
            .ids()
            .find(|&id| tab.pane(id).buffer == buffer)
            .expect("the buffer is in a pane");
        tab.pane(id)
    }

    /// The pane showing `buffer`, writable.
    pub fn pane_mut_for(&mut self, buffer: BufferId) -> &mut Pane {
        let tab = &mut self.tabs[self.active];
        let id = tab
            .ids()
            .find(|&id| tab.pane(id).buffer == buffer)
            .expect("the buffer is in a pane");
        tab.pane_mut(id)
    }

    /// A pane's buffer and viewport together.
    pub fn pane_mut(&mut self, id: PaneId) -> (&mut Buffer, &mut Viewport) {
        let pane = self.tabs[self.active].pane_mut(id);
        let buffer = &mut self.buffers[pane.buffer.0];
        (buffer, &mut pane.viewport)
    }

    /// Puts a buffer beside the list, splitting the tab if needed.
    pub fn show(&mut self, buffer: Buffer) {
        self.selection = None;
        // Reuse the existing slot so we don't accumulate every file ever opened.
        let id = match self.tabs[self.active].right_pane_buffer() {
            Some(id) => {
                self.buffers[id.0] = buffer;
                id
            }
            None => {
                self.buffers.push(buffer);
                BufferId(self.buffers.len() - 1)
            }
        };
        self.version = Version(self.version.0 + 1);
        self.tabs[self.active].set_right_pane(id);
    }

    /// Replaces the explorer's file list, preserving cursor position.
    pub fn update_explorer(&mut self, files: Vec<file_types::File>) {
        let explorer_id = BufferId(0); // The explorer is always the first buffer.
        let buffer = &mut self.buffers[explorer_id.0];
        let tab = &mut self.tabs[self.active];
        let pane_id = tab.ids().next().unwrap();
        let pane = tab.pane_mut(pane_id);
        let cursor = pane.viewport.cursor();
        if let BufferType::Explorer(explorer) = buffer.buffer_type_mut() {
            let landing = explorer.reshape_around(cursor, |e| e.refresh(files));
            let lines = explorer.view_lines();
            pane.viewport.place(landing, lines);
        }
        self.version = Version(self.version.0 + 1);
    }

    /// The focused pane's buffer and viewport together.
    pub fn focused_mut(&mut self) -> (&mut Buffer, &mut Viewport) {
        let pane = self.tabs[self.active].focused_mut();
        let buffer = &mut self.buffers[pane.buffer.0];
        (buffer, &mut pane.viewport)
    }

    /// Switches the diff between side-by-side and inline, keeping the cursor
    /// on the same file line.
    pub fn toggle_layout(&mut self) {
        self.selection = None;
        let Some(pane) = self.reading() else {
            return;
        };
        let id = self.tabs[self.active].pane(pane).buffer.0;
        let cursor = self.tabs[self.active].pane(pane).viewport.cursor();

        let anchor = self.line_at(id, cursor);
        let flipped = self.buffers.remove(id).switch_diff_layout();
        self.buffers.insert(id, flipped);

        let landing = anchor.and_then(|(version, line)| {
            let buffer = &self.buffers[id];
            let view_line = buffer
                .alignment()?
                .view_line_at(buffer.diff_type()?, version, line)?;
            Some((view_line, buffer.view_lines()))
        });
        if let Some((view_line, total)) = landing {
            self.tabs[self.active]
                .pane_mut(pane)
                .viewport
                .place(view_line, total);
        }
    }

    /// The pane showing a diff, which is the one a layout key means.
    ///
    /// The focused pane when it is a diff, and otherwise the only other pane
    /// there is. `None` when nothing on screen has two versions to lay out.
    fn reading(&self) -> Option<PaneId> {
        let tab = self.tab();
        let is_diff = |id: PaneId| {
            self.buffers[tab.pane(id).buffer.0]
                .diff_type()
                .is_some_and(DiffType::is_diff)
        };
        let focus = tab.focus();
        if is_diff(focus) {
            return Some(focus);
        }
        tab.ids().find(|&id| is_diff(id))
    }

    fn line_at(&self, id: usize, view_line: u32) -> Option<(align::DiffVersion, u32)> {
        let buffer = &self.buffers[id];
        buffer.alignment()?.line_at(buffer.diff_type()?, view_line)
    }

    pub fn overlays(&self) -> &[Overlay] {
        &self.overlays
    }
}
