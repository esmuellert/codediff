//! One pane: a buffer, drawn where the tab put it.
//!
//! The level that turns a rectangle into a height. Nothing below is told how
//! many panes there are, and nothing above is told what type of buffer this
//! is.

use ratatui::buffer::Buffer as Cells;
use ratatui::layout::Rect;

use crate::draw::{Look, buffer};
use crate::view::{PaneId, View};

/// Draws one pane of the tab into `rect`.
pub fn draw(cells: &mut Cells, rect: Rect, view: &mut View, id: PaneId, look: Look<'_>) -> bool {
    let focused = view.tab().focus() == id;
    let (shown, viewport) = view.pane_mut(id);
    viewport.set_height(u32::from(rect.height), shown.view_lines());
    buffer::draw(cells, rect, shown, viewport, look, focused)
}
