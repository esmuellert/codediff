//! The whole screen: the body, and the status line under it.
//!
//! The outermost renderer, and the one place a buffer kind is dispatched on
//! for drawing. Adding a kind is a new arm here and a new file beside this
//! one, and the compiler names the arm that is missing.
//!
//! One pane fills the body today. When a tab holds two — the explorer beside a
//! diff — this becomes a walk of the tab's rectangles, and nothing below it
//! changes.

use ratatui::buffer::Buffer as Cells;
use ratatui::layout::Rect;

use crate::draw::{Look, inline, side_by_side, single_file, status};
use crate::render::{cells, layout};
use crate::syntax::Store;
use crate::theme::Theme;
use crate::view::Buffer;
use crate::view::buffer::BufferType;
use crate::view::{View, Viewport};

/// Renders the whole interface into the terminal's cell grid.
///
/// `view` is taken by mutable reference for one reason: the frame is where a
/// pane's height becomes known, and page motions need it. A terminal resize
/// therefore needs no event of its own — the next frame simply has a different
/// height, and the viewport re-examines itself when told.
pub fn render(cells: &mut Cells, area: Rect, view: &mut View, theme: &Theme, store: &Store) {
    let look = Look {
        theme,
        syntax: view.syntax(),
        store,
    };
    let Some((body, status_area)) = layout::screen(area) else {
        return too_small(cells, area, theme);
    };

    // One pane fills the body. When `Layout` grows a variant this becomes a
    // walk of the tab's rectangles, and nothing below it changes.
    let rect = body;

    let (buffer, viewport) = view.focused_mut();
    viewport.set_height(u32::from(rect.height), buffer.view_lines());
    if !pane(cells, rect, buffer, viewport, look) {
        return too_small(cells, area, theme);
    }

    let (buffer, viewport) = view.focused_mut();
    status::draw(cells, status_area, &summary(buffer, viewport), theme);
}

/// Draws one pane's buffer, whatever kind it is.
///
/// The only place a buffer kind is examined for drawing. Returns `false` if
/// the pane is too small to draw meaningfully.
fn pane(
    cells: &mut Cells,
    area: Rect,
    buffer: &Buffer,
    viewport: &Viewport,
    look: Look<'_>,
) -> bool {
    // The one place a buffer kind decides which renderer runs. Side by side
    // and inline are separate variants, so the layout needs no field of its
    // own to be read here.
    match buffer.buffer_type() {
        BufferType::SideBySide(data) => {
            side_by_side::draw(cells, area, buffer, data, viewport, look)
        }
        BufferType::Inline(data) => inline::draw(cells, area, buffer, data, viewport, look),
        BufferType::SingleFile(data) => single_file::draw(cells, area, data, viewport, look),
    }
}

/// What the status line says about the focused pane.
fn summary<'a>(buffer: &'a Buffer, viewport: &Viewport) -> status::Status<'a> {
    // Every field comes off the parent, whatever kind it is: a lone file
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
