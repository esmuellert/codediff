//! The whole screen: the body, and the status line under it.
//!
//! The outermost level. It knows there is a body and a status line, and
//! nothing about what is in either.

use ratatui::buffer::Buffer as Cells;
use ratatui::layout::Rect;

use crate::draw::screen_map::ScreenMap;
use crate::draw::{Look, status, tab};
use crate::render::{cells, layout};
use crate::theme::Theme;
use crate::view::{Buffer, View, Viewport};
use syntax::Store;

/// Renders the whole interface into the terminal's cell grid.
pub fn render(
    cells: &mut Cells,
    area: Rect,
    view: &mut View,
    theme: &Theme,
    store: &Store,
    notice: Option<&str>,
    screen_map: &mut ScreenMap,
) {
    screen_map.clear();
    let look = Look {
        theme,
        syntax: view.syntax(),
        store,
    };
    let Some((body, status_area)) = layout::screen(area) else {
        return too_small(cells, area, theme);
    };
    screen_map.body = body;

    if !tab::draw(cells, body, view, look, screen_map) {
        return too_small(cells, area, theme);
    }

    let (buffer, viewport) = view.focused_mut();
    status::draw(
        cells,
        status_area,
        &summary(buffer, viewport, notice),
        theme,
    );
}

/// What the status line says about the focused pane.
fn summary<'a>(
    buffer: &'a Buffer,
    viewport: &Viewport,
    notice: Option<&'a str>,
) -> status::Status<'a> {
    // Every field comes off the parent, whatever type it is: a lone file
    // reports no changes because it has none, not because this asked a
    // different question of it.
    status::Status {
        file: buffer.file(),
        view_line: viewport.cursor(),
        view_lines: buffer.view_lines(),
        changes: buffer.blocks().len(),
        change: buffer.block_at(viewport.cursor()),
        timed_out: buffer.hit_timeout(),
        exhausted: buffer.exhausted(),
        notice,
    }
}

fn too_small(cells: &mut Cells, area: Rect, theme: &Theme) {
    let style = theme.normal;
    for y in area.y..area.bottom() {
        cells::fill(
            cells,
            Rect {
                y,
                height: 1,
                ..area
            },
            style,
        );
    }
    if area.height > 0 {
        cells::write(
            cells,
            Rect {
                y: area.y,
                height: 1,
                ..area
            },
            0,
            "terminal too small",
            style,
        );
    }
}
