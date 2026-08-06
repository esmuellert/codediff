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

use file_types::DiffType;

use crate::input::KeymapType;
use crate::syntax::{Store, Syntax, Version};

/// A buffer's place in [`View::buffers`].
///
/// An index rather than a reference, so panes can name buffers without
/// borrowing them and the whole tree stays movable.
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
    /// Floating layers, drawn over the tabs, topmost last.
    ///
    /// Empty, and the reason it exists now is that event routing changes shape
    /// when the first one arrives: overlays are offered keys before the
    /// focused pane. Doing that once is cheaper than doing it twice.
    overlays: Vec<Overlay>,
    /// Whether the code is coloured by its language.
    ///
    /// One switch for the session rather than one per buffer: see
    /// [`ViewAction::ToggleSyntax`](crate::input::ViewAction::ToggleSyntax).
    syntax: bool,
    /// Which content the open files are at.
    ///
    /// One number for all of them, raised whenever a buffer is replaced.
    /// **A file's name does not change when its bytes do** — the working tree
    /// has no id git can give it — so the colour store would otherwise answer
    /// a re-read with the colours of what the file used to be. That is the
    /// same fault as the diff cache D51 removed, one layer up.
    ///
    /// Raising it for every open discards a little more than it must, since
    /// only one file was re-read. Colouring one file is what happens on any
    /// first open, so the cost is a frame; keeping this per file needs a
    /// watcher to say which file moved, which is S14.
    version: Version,
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
        let (start, rows) = (buffer.start_row(), buffer.view_lines());
        let mut view = Self {
            buffers: vec![buffer],
            tabs: vec![Tab::single(BufferId(0))],
            active: 0,
            overlays: Vec::new(),
            syntax: true,
            version: Version(1),
        };
        // The buffer decides where a reader starts, and the viewport is what
        // holds the answer — so the two are set together, here, rather than
        // left for the first frame to reconcile.
        view.tabs[0].focused_mut().viewport.jump(start, rows);
        view
    }

    /// Whether code is being coloured by its language.
    pub fn syntax(&self) -> bool {
        self.syntax
    }

    pub fn toggle_syntax(&mut self) {
        self.syntax = !self.syntax;
    }

    /// Asks for the colours of everything on screen, and a little beyond.
    ///
    /// Called after anything that can change what is visible — opening a
    /// buffer, scrolling, toggling the layout. Cheap and usually silent: the
    /// store answers most of the time, and a request goes out only when the
    /// reader has moved past what has been coloured.
    ///
    /// **The margin is what keeps scrolling smooth.** Asking only for the
    /// screen would mean a request every time a line came into view, each
    /// waiting on the one before. Two thousand lines is one chunk of the
    /// worker's work, so an ordinary scroll finds its colours already there.
    pub fn request(&mut self, syntax: &mut Syntax, store: &mut Store) {
        const MARGIN: u32 = 2_000;
        let version = self.version;
        // **Every pane, not the focused one.** Both are on screen, and what is
        // on screen is what needs colouring — a diff beside a list is not
        // being read any less because the reader's keys are going elsewhere.
        // Asking only for the focused pane left the diff in plain text for as
        // long as the list had focus, which is most of the time.
        let panes: Vec<PaneId> = self.tab().ids().collect();
        for id in panes {
            let (buffer, viewport) = self.pane_mut(id);
            // View lines, not file lines. Filler rows make a view line number
            // at least its file line number, so this over-asks slightly and
            // never under-asks; the buffer clamps it to the length of each
            // side.
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

    /// The pane showing `buffer`, read-only.
    pub fn pane_for(&self, buffer: BufferId) -> &Pane {
        let tab = self.tab();
        let id = tab
            .ids()
            .find(|&id| tab.pane(id).buffer == buffer)
            .expect("the buffer is in a pane");
        tab.pane(id)
    }

    /// The pane showing `buffer`, so a caller can put it back where it was.
    ///
    /// By buffer rather than by [`PaneId`], because what a caller has after
    /// showing something is the buffer it showed.
    pub fn pane_mut_for(&mut self, buffer: BufferId) -> &mut Pane {
        let tab = &mut self.tabs[self.active];
        let id = tab
            .ids()
            .find(|&id| tab.pane(id).buffer == buffer)
            .expect("the buffer is in a pane");
        tab.pane_mut(id)
    }

    /// One pane's buffer and viewport, together.
    ///
    /// The same pair as [`focused_mut`](Self::focused_mut), for a pane that is
    /// named rather than focused — which is what drawing needs, since it draws
    /// every pane and not only the one in use.
    pub fn pane_mut(&mut self, id: PaneId) -> (&mut Buffer, &mut Viewport) {
        let pane = self.tabs[self.active].pane_mut(id);
        let buffer = &mut self.buffers[pane.buffer.0];
        (buffer, &mut pane.viewport)
    }

    /// Puts a buffer beside the list, splitting the tab if it is whole.
    ///
    /// The buffer is added to the view and the tab is told which it is. Both
    /// steps are here because [`View`] owns the buffers and the tab owns the
    /// arrangement, and this is the lowest level holding the two.
    pub fn show(&mut self, buffer: Buffer) {
        // The slot the tab is already pointing at, if it has one. Pushing
        // every time would leave the file the reader has moved on from held
        // for ever, and a reviewer opening two hundred files would carry all
        // two hundred.
        let id = match self.tabs[self.active].shown() {
            Some(id) => {
                self.buffers[id.0] = buffer;
                id
            }
            None => {
                self.buffers.push(buffer);
                BufferId(self.buffers.len() - 1)
            }
        };
        // The bytes behind a name may have changed, and the name will not say
        // so. Raising the version is what makes the store discard what it has
        // rather than draw the old colours over the new lines.
        self.version = Version(self.version.0 + 1);
        self.tabs[self.active].show(id);
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
        // The diff, whichever pane it is in. A list has no layout to flip, so
        // pressing this with the list focused used to do nothing at all —
        // a silent key, which is the failure the keymap exists to prevent.
        // There is only ever one diff on screen, so "the diff" is unambiguous.
        let Some(pane) = self.reading() else {
            return;
        };
        let id = self.tabs[self.active].pane(pane).buffer.0;
        let cursor = self.tabs[self.active].pane(pane).viewport.cursor();

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
                .view_line_at(buffer.diff_type()?, version, line)?;
            Some((view_line, buffer.view_lines()))
        });
        if let Some((view_line, total)) = landing {
            self.tabs[self.active]
                .pane_mut(pane)
                .viewport
                .jump(view_line, total);
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
                .is_some_and(DiffType::is_paired)
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
