//! Everything on screen, and where it sits.
//!
//! ---
//!
//! Admission criterion: does this decide *what is visible and where*? Four
//! levels, each containing the next, and one file each — so the module tree
//! and the model are the same picture:
//!
//! ```text
//! view/            View     tabs, and every buffer any of them can show
//! ├ tab.rs         Tab      a layout of panes, and which has focus
//! ├ pane.rs        Pane     one buffer, and one Viewport onto it
//! ├ viewport.rs    Viewport top, cursor, left
//! └ buffer/        Buffer   what a pane can show
//! ```
//!
//! `buffer/` is *inside* this module because [`View`] owns the buffers.
//! Neovim's are global and Helix keeps `documents` beside its `tree`, but both
//! have an editor above that owns the two; we do not, and inventing one to
//! justify a directory would be the tail wagging the dog.
//!
//! Buffers live in [`View`], not in the panes that show them, so two panes can
//! show one buffer and neither owns it. Panes refer to them by [`BufferId`] —
//! never by reference, which would make the whole structure self-referential.
//! Helix does exactly this with `DocumentId`/`ViewId`; Zellij's `Box<dyn
//! Pane>` is the counter-example, and forced `Rc<RefCell<_>>` throughout.
//!
//! The containment order is also the **execution order**: an action is carried
//! out by the lowest level that contains everything it affects. A motion
//! affects one viewport, so the pane's buffer does it. Resizing a border
//! affects two panes, so the tab must. See D27.

pub mod buffer;
mod pane;
mod tab;
mod viewport;

pub use buffer::{Buffer, BufferType, Direction};
pub use pane::Pane;
pub use tab::{Layout, PaneId, Tab};
pub use viewport::Viewport;

use crate::input::KeymapType;

/// A buffer's place in [`View::buffers`].
///
/// An index rather than a reference, so panes can name buffers without
/// borrowing them and the whole tree stays movable.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct BufferId(usize);

/// Everything the interface is showing.
#[derive(Debug)]
pub struct View {
    buffers: Vec<Buffer>,
    tabs: Vec<Tab>,
    active: usize,
    /// Floating layers, drawn over the tabs, topmost last.
    ///
    /// Empty, and the reason it exists now is that event routing changes shape
    /// when the first one arrives: overlays are offered keys before the
    /// focused pane. Doing that once is cheaper than doing it twice.
    overlays: Vec<Overlay>,
}

/// A floating layer over the tabs.
///
/// Deliberately uninhabited — help and prompts arrive with the explorer. The
/// stack and its routing exist first so adding one is an addition.
#[derive(Debug)]
pub enum Overlay {}

impl View {
    /// Opens one buffer in one pane in one tab.
    pub fn single(buffer: Buffer) -> Self {
        Self {
            buffers: vec![buffer],
            tabs: vec![Tab::single(BufferId(0))],
            active: 0,
            overlays: Vec::new(),
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

    /// What the reader is looking at.
    ///
    /// The read-only counterpart of [`focused_mut`](Self::focused_mut), which
    /// has to hand out both halves at once because a buffer acts on a viewport
    /// it does not own.
    pub fn focused_buffer(&self) -> &Buffer {
        self.buffer(self.focused().buffer)
    }

    /// The keymap that is live, which the focused buffer decides.
    pub fn keymap_type(&self) -> KeymapType {
        self.buffer(self.focused().buffer).keymap_type()
    }

    /// The focused pane's buffer and viewport, together.
    ///
    /// Returned as a pair because a buffer acts on a viewport it does not own,
    /// and both borrows have to be taken at once.
    pub fn focused_mut(&mut self) -> (&mut Buffer, &mut Viewport) {
        let pane = self.tabs[self.active].focused_mut();
        let buffer = &mut self.buffers[pane.buffer.0];
        (buffer, &mut pane.viewport)
    }

    /// Reads the focused diff the other way round, keeping the reader's place.
    ///
    /// Here rather than on the buffer because the layout decides what a view
    /// line *is*, so the pane's cursor has to be translated at the same moment
    /// the buffer changes it. The view is the lowest level holding both.
    ///
    /// A buffer with no second version has only one way to be read, and is
    /// left alone.
    pub fn toggle_layout(&mut self) {
        let id = self.focused().buffer.0;
        let cursor = self.focused().viewport.cursor();

        // The view line the cursor is on means nothing in the other layout;
        // the file line it shows means the same in both, so that is what is
        // carried across.
        let anchor = self.line_at(id, cursor);
        // Taken out and put back rather than mutated: the view-line count and
        // the changed blocks both follow from the layout, so the buffer is
        // rebuilt. `BufferId` is an index, so it must land where it was.
        let flipped = self.buffers.remove(id).flipped();
        self.buffers.insert(id, flipped);

        let landing = anchor.and_then(|(version, line)| {
            let buffer = &self.buffers[id];
            let view_line = buffer
                .alignment()?
                .view_line_at(buffer.layout()?, version, line)?;
            Some((view_line, buffer.view_lines()))
        });
        if let Some((view_line, total)) = landing {
            self.tabs[self.active]
                .focused_mut()
                .viewport
                .jump(view_line, total);
        }
    }

    fn line_at(&self, id: usize, view_line: u32) -> Option<(align::DiffVersion, u32)> {
        let buffer = &self.buffers[id];
        buffer.alignment()?.line_at(buffer.layout()?, view_line)
    }

    pub fn overlays(&self) -> &[Overlay] {
        &self.overlays
    }
}
