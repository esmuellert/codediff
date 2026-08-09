//! One tab: every pane it holds, and the border between two.
//!
//! A tab holds one pane or two, and this walks whichever it has. Nothing below
//! is told how many there are — each pane is handed a rectangle and draws into
//! it.

use ratatui::buffer::Buffer as Cells;
use ratatui::layout::Rect;

use crate::draw::screen_map::ScreenMap;
use crate::draw::{Look, pane};
use crate::render::{cells, layout};
use crate::view::{Layout, PaneId, View};

/// Draws every pane the tab has, in its own rectangle.
pub fn draw(
    cells: &mut Cells,
    body: Rect,
    view: &mut View,
    look: Look<'_>,
    screen_map: &mut ScreenMap,
) -> bool {
    let places = match view.tab().layout() {
        Layout::Split { left } => layout::split(body, left),
        Layout::Full => None,
    };
    let Some((left_area, border, right_area)) = places else {
        let focus = view.tab().focus();
        screen_map.panes.push((focus, body));
        return pane::draw(cells, body, view, focus, look, screen_map);
    };

    // A row at a time: `hatch` draws one row, and handing it a full-height
    // rectangle drew the border only on the top line — as `side_by_side` does
    // for the divider between its own two columns.
    let style = look.theme.normal.patch(look.theme.divider);
    for y in border.y..border.bottom() {
        cells::hatch(
            cells,
            Rect {
                y,
                height: 1,
                ..border
            },
            "│",
            style,
        );
    }

    // A pane that cannot draw falls back to one pane rather than failing the
    // whole screen. Whether a diff fits depends on how wide its line numbers
    // are, which the rectangle arithmetic cannot know, so the only place that
    // can answer "does this fit" is the attempt — and a screen that says
    // "terminal too small" while the list beside it would have drawn perfectly
    // is worse than the list on its own.
    let panes: Vec<PaneId> = view.tab().ids().collect();
    let fits = panes.iter().enumerate().all(|(index, &id)| {
        let rect = if index == 0 { left_area } else { right_area };
        screen_map.panes.push((id, rect));
        pane::draw(cells, rect, view, id, look, screen_map)
    });
    if fits {
        return true;
    }
    // Nothing is cleared first: every renderer fills the rows it is given
    // edge to edge, so the second attempt covers the border and whatever the
    // first attempt had drawn. Clearing here as well was measurably
    // redundant — removing it changed no test and no frame.
    screen_map.clear();
    let focus = view.tab().focus();
    screen_map.panes.push((focus, body));
    pane::draw(cells, body, view, focus, look, screen_map)
}
