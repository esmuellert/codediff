//! Drawing dispatch: one file draws one buffer type.

mod explorer;
mod inline;
mod side_by_side;
mod single_file;

use ratatui::buffer::Buffer as Cells;
use ratatui::layout::Rect;

use crate::draw::{Look, TextRects};
use crate::view::buffer::BufferType;
use crate::view::{Buffer, Viewport};

/// Draws one pane's buffer, whatever type it is.
///
/// Returns `None` if the pane is too small to draw, or the text rects drawn.
pub fn draw(
    cells: &mut Cells,
    area: Rect,
    buffer: &Buffer,
    viewport: &Viewport,
    look: Look<'_>,
    focused: bool,
) -> Option<TextRects> {
    match buffer.buffer_type() {
        BufferType::SideBySide(data) => {
            side_by_side::draw(cells, area, buffer, data, viewport, look)
        }
        BufferType::Inline(data) => inline::draw(cells, area, buffer, data, viewport, look),
        BufferType::SingleFile(data) => single_file::draw(cells, area, data, viewport, look),
        BufferType::Explorer(data) => {
            explorer::draw(cells, area, data, viewport, look.theme, focused)
        }
    }
}
