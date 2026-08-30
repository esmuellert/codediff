//! One tab: every pane it holds, each in a box of its own.
//!
//! A tab holds one pane or two, and this walks whichever it has. Nothing below
//! is told how many there are — each pane is handed the rectangle inside its
//! box and draws into it.

use ratatui::buffer::Buffer as Cells;
use ratatui::layout::Rect;
use ratatui::style::Style;

use crate::draw::screen_map::ScreenMap;
use crate::draw::{Look, pane};
use crate::render::{border, cells, layout};
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
    let Some((list, divider, rest)) = places else {
        return draw_box(cells, body, view, look, screen_map);
    };

    // Two boxes touching — no gap. The left box absorbs the divider column.
    let boxes = [
        Rect {
            width: list.width + divider.width,
            ..list
        },
        rest,
    ];
    let boxed = can_border(body);
    let insides = if boxed {
        boxes.map(|rect| apply_padding(cells, border::inner(rect), look))
    } else {
        [list, rest]
    };

    let ids: Vec<PaneId> = view.tab().ids().collect();
    if boxed {
        draw_borders(cells, &ids, boxes, view.tab().focus(), look);
    } else {
        let divider_style = look.theme.normal.patch(look.theme.divider);
        for y in divider.y..divider.bottom() {
            cells::fill_repeat_pattern(
                cells,
                Rect {
                    y,
                    height: 1,
                    ..divider
                },
                "│",
                divider_style,
            );
        }
    }

    // If a pane cannot fit its content, fall back to one pane filling the body.
    let fits = ids.iter().enumerate().all(|(index, &id)| {
        let rect = if index == 0 { insides[0] } else { insides[1] };
        screen_map.panes.push((id, rect));
        pane::draw(cells, rect, view, id, look, screen_map)
    });
    if fits {
        return true;
    }
    screen_map.clear();
    draw_box(cells, body, view, look, screen_map)
}

/// Draws the tab as the focused pane draw_box, filling the body.
fn draw_box(
    cells: &mut Cells,
    body: Rect,
    view: &mut View,
    look: Look<'_>,
    screen_map: &mut ScreenMap,
) -> bool {
    let focus = view.tab().focus();
    let rect = if can_border(body) {
        border::draw(cells, body, border_style(look, true));
        apply_padding(cells, border::inner(body), look)
    } else {
        body
    };
    screen_map.panes.push((focus, rect));
    pane::draw(cells, rect, view, focus, look, screen_map)
}

/// Keeps a column clear either side of a pane, so its text does not touch the
/// box. The inside is filled first, since nothing else paints those columns.
///
/// A box with two columns or fewer inside it has none to spare, and is handed
/// back whole rather than made unusable.
fn apply_padding(cells: &mut Cells, inside: Rect, look: Look<'_>) -> Rect {
    for y in inside.y..inside.bottom() {
        let row = Rect {
            y,
            height: 1,
            ..inside
        };
        cells::fill(cells, row, look.theme.normal);
    }
    if inside.width <= 2 {
        return inside;
    }
    Rect {
        x: inside.x + 1,
        width: inside.width - 2,
        ..inside
    }
}

/// Draws a box round each pane.
fn draw_borders(cells: &mut Cells, ids: &[PaneId], boxes: [Rect; 2], focus: PaneId, look: Look<'_>) {
    for (&id, rect) in ids.iter().zip(boxes) {
        border::draw(cells, rect, border_style(look, id == focus));
    }
}

/// The colour of a box, focused or not.
fn border_style(look: Look<'_>, focused: bool) -> Style {
    let border = if focused {
        look.theme.border_focused
    } else {
        look.theme.border
    };
    look.theme.normal.patch(border)
}

/// Whether the body can hold a box with a pane inside it.
fn can_border(body: Rect) -> bool {
    body.width >= border::MIN && body.height >= border::MIN
}
