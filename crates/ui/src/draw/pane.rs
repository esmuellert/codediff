//! One pane: a buffer, drawn where the tab put it.
//!
//! The level that turns a rectangle into a height. Nothing below is told how
//! many panes there are, and nothing above is told what type of buffer this
//! is.

use ratatui::buffer::Buffer as Cells;
use ratatui::layout::Rect;

use crate::draw::screen_map::{ScreenMap, TextArea};
use crate::draw::{Look, buffer};
use crate::render::selection as render_sel;
use crate::view::{PaneId, View};

/// Draws one pane of the tab into `rect`.
pub fn draw(
    cells: &mut Cells,
    rect: Rect,
    view: &mut View,
    id: PaneId,
    look: Look<'_>,
    screen_map: &mut ScreenMap,
) -> bool {
    let focused = view.tab().focus() == id;
    let (shown, viewport) = view.pane_mut(id);
    viewport.set_height(u32::from(rect.height), shown.view_lines());
    let Some(text_rects) = buffer::draw(cells, rect, shown, viewport, look, focused) else {
        return false;
    };

    // Record text areas for mouse hit-testing.
    for &(column, rect) in &text_rects {
        screen_map.text_areas.push(TextArea {
            pane: id,
            column,
            rect,
        });
    }

    // Apply selection highlight if this pane owns it.
    if let Some((pane_id, ref sel)) = view.selection
        && pane_id == id
    {
        let viewport = &view.tab().pane(id).viewport;
        let style = look.theme.selection;
        if let Some(&(_, text_rect)) = text_rects.iter().find(|(col, _)| *col == sel.column) {
            let start = sel.start_pos();
            let end = sel.end_pos();
            render_sel::overlay(
                cells,
                text_rect,
                viewport.top(),
                viewport.left(),
                render_sel::Range {
                    start_line: start.line,
                    start_col: start.col,
                    end_line: end.line,
                    end_col: end.col,
                },
                style,
            );
        }
    }

    true
}
