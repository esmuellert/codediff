//! Drawing dispatch: one file draws one buffer type.
//!
//! ```text
//! view/buffer/side_by_side.rs  ←→  draw/buffer/side_by_side.rs
//! view/buffer/inline.rs        ←→  draw/buffer/inline.rs
//! view/buffer/single_file.rs   ←→  draw/buffer/single_file.rs
//! view/buffer/explorer/       ←→  draw/buffer/explorer/
//! ```

mod explorer;
mod inline;
mod side_by_side;
mod single_file;

use ratatui::buffer::Buffer as Cells;
use ratatui::layout::Rect;

use crate::draw::Look;
use crate::view::buffer::BufferType;
use crate::view::{Buffer, Viewport};

/// Draws one pane's buffer, whatever type it is.
///
/// The only place a buffer type is examined for drawing. Returns `false` if
/// the pane is too small to draw meaningfully.
pub fn draw(
    cells: &mut Cells,
    area: Rect,
    buffer: &Buffer,
    viewport: &Viewport,
    look: Look<'_>,
    focused: bool,
) -> bool {
    // The one place a buffer type decides which renderer runs. Side by side
    // and inline are separate variants, so the layout needs no field of its
    // own to be read here.
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
