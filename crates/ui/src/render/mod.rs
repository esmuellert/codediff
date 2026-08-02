//! Turning a view into a grid of cells.
//!
//! Nothing here holds state. A frame is a pure function of the view and the
//! theme, which is why the whole interface can be tested by rendering into a
//! buffer and reading the text back out.
//!
//! Five levels, the top three matching the view's own:
//!
//! ```text
//! mod             the screen: body and status line
//! side_by_side    one pane holding a diff in two columns
//! text            one pane holding a single file
//! column          one gutter-and-text column of a diff
//! cells           writing characters, tabs and wide glyphs into a rect
//! ```
//!
//! The two pane renderers are named for the buffer kinds they draw, so the two
//! folders line up one for one:
//!
//! ```text
//! view/buffer/side_by_side.rs  ←→  render/side_by_side.rs
//! view/buffer/text.rs          ←→  render/text.rs
//! ```
//!
//! A buffer kind is dispatched on exactly once, in [`pane`]. Adding one is a
//! new arm and a new file, and the compiler names the arm that is missing.

mod cells;
mod column;
pub mod layout;
mod side_by_side;
mod status;
mod text;

use ratatui::buffer::Buffer as Cells;
use ratatui::layout::Rect;

use crate::theme::Theme;
use crate::view::Buffer;
use crate::view::{View, Viewport};

/// Draws the whole interface.
///
/// `view` is taken by mutable reference for one reason: the frame is where a
/// pane's height becomes known, and page motions need it. A terminal resize
/// therefore needs no event of its own — the next frame simply has a different
/// height, and the viewport re-examines itself when told.
pub fn draw(cells: &mut Cells, area: Rect, view: &mut View, theme: &Theme) {
    let Some((body, status_area)) = layout::screen(area) else {
        return too_small(cells, area, theme);
    };

    // One pane fills the body. When `Layout` grows a variant this becomes a
    // walk of the tab's rectangles, and nothing below it changes.
    let rect = body;

    let (buffer, viewport) = view.focused_mut();
    viewport.set_height(u32::from(rect.height), buffer.rows());
    if !pane(cells, rect, buffer, viewport, theme) {
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
    theme: &Theme,
) -> bool {
    match buffer {
        Buffer::SideBySide(data) => side_by_side::draw(cells, area, data, viewport, theme),
        Buffer::Text(data) => text::draw(cells, area, data, viewport, theme),
    }
}

/// What the status line says about the focused pane.
fn summary<'a>(buffer: &'a Buffer, viewport: &Viewport) -> status::Status<'a> {
    let (changes, change, timed_out) = match buffer {
        Buffer::SideBySide(data) => (
            data.blocks().len(),
            data.block_at(viewport.cursor()),
            data.hit_timeout(),
        ),
        Buffer::Text(_) => (0, None, false),
    };
    status::Status {
        path: buffer.label(),
        row: viewport.cursor(),
        rows: buffer.rows(),
        changes,
        change,
        timed_out,
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
