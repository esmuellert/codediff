//! Drawing the list of changed files.
//!
//! ```text
//! mod.rs        the pane: which lines are on screen, and what precedes each
//! tree.rs       the nested arrangement's indent guides and fold arrows
//! view_line.rs  one line: its text, its colour, and where each piece goes
//! ```
//!
//! `view_line.rs` is named for what it draws, and one function in it per
//! variant of [`ViewLine`]. It was `node.rs`, which was wrong twice over: a
//! heading has no node behind it, and the file was named for the tree rather
//! than for what it makes.
//!
//! [`ViewLine`]: crate::view::buffer::explorer::ViewLine
//!
//! **Nothing here is reusable, and that is the point.** The order of a line —
//! guides, name, where it moved from, what it gained, what happened to it — is
//! the file list and nothing else. A tree of commits would be graph lanes, a
//! subject and an author, and would write its own files beside these. What
//! both would share is `line_index`, which counts columns for everyone.
//!
//! **The flat arrangement has no file of its own**, because it needs nothing
//! in front of a line. There was one, and it was dead code: it asked whether a
//! node was a directory, in an arrangement that has none. See D69.
//!
//! Which line is which is already settled by the time anything here runs: the
//! viewport needs the count before a frame to clamp the cursor, so the walk
//! that produced it happened in [`view`](crate::view), on the keypress that
//! changed it.

mod tree;
mod view_line;

use ratatui::buffer::Buffer as Cells;
use ratatui::layout::Rect;

use crate::render::cells;
use crate::theme::Theme;
use crate::view::Viewport;
use crate::view::buffer::Explorer;

/// Draws the list into `area`.
///
/// Returns `false` if the pane is too narrow to say anything, which the caller
/// reports rather than filling with cut-off fragments.
pub fn draw(
    cells: &mut Cells,
    area: Rect,
    explorer: &Explorer,
    viewport: &Viewport,
    theme: &Theme,
    focused: bool,
) -> bool {
    if area.width < 4 || area.height == 0 {
        return false;
    }
    let visible = viewport.visible(explorer.view_lines());
    for (offset, y) in (area.y..area.bottom()).enumerate() {
        let line = Rect {
            y,
            height: 1,
            ..area
        };
        let index = visible.start + offset as u32;
        let selected = focused && index == viewport.cursor();
        let background = if selected {
            theme.cursor_line
        } else {
            theme.normal
        };
        cells::fill(cells, line, background);

        let Some(view_line) = explorer.view_line(index) else {
            continue;
        };
        // Guides come from the arrangement that has ancestors, and only from
        // it: a heading has no indent to describe, and a flat list has none to
        // read.
        let prefix = match explorer.nested_at(index) {
            Some((model, id)) => tree::prefix(model, id, theme, background),
            None => Vec::new(),
        };
        view_line::draw(cells, line, &view_line, prefix, theme, background);
    }
    true
}
