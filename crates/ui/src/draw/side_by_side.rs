//! Drawing a side-by-side diff into one pane.
//!
//! Reads the [`Alignment`] the buffer already holds. Nothing here builds one:
//! the pipeline did that once, when the file was opened, and a frame that
//! rebuilt it would be redoing work whose inputs cannot have changed.
//!
//! [`Alignment`]: align::Alignment

use align::{DiffLayout, DiffVersion};
use ratatui::buffer::Buffer as Cells;
use ratatui::layout::Rect;

use crate::render::layout;
use crate::render::line::Painter;
use crate::render::{cells, column};
use crate::theme::Theme;
use crate::view::Viewport;
use crate::view::buffer::Buffer;
use crate::view::buffer::SideBySide;

/// Draws one diff into the pane's area.
///
/// Returns `false` if the pane is too narrow to draw, which the caller shows
/// as a message rather than a corrupt frame.
pub fn draw(
    buf: &mut Cells,
    area: Rect,
    buffer: &Buffer,
    data: &SideBySide,
    view: &Viewport,
    theme: &Theme,
) -> bool {
    let alignment = data.alignment();
    let Some(frame) = layout::columns(
        area,
        data.divider(),
        alignment.lines(DiffVersion::Original).len() as u32,
        alignment.lines(DiffVersion::Modified).len() as u32,
    ) else {
        return false;
    };

    let visible = view.visible(buffer.view_lines());

    // Collected once and handed to both columns. Two columns reading one slice
    // cannot disagree about what line they are on.
    let lines: Vec<_> = alignment
        .view_lines_from(DiffLayout::SideBySide, visible.start)
        .take(visible.len())
        .collect();

    let painter = Painter {
        alignment,
        theme,
        top: visible.start,
        cursor: view.cursor(),
        left: view.left(),
    };
    for (side, column) in frame.columns() {
        column::draw(buf, column, side, &lines, painter);
    }

    let area = frame.divider;
    let style = theme.normal.patch(theme.divider);
    for y in area.y..area.bottom() {
        cells::hatch(
            buf,
            Rect {
                y,
                height: 1,
                ..area
            },
            "│",
            style,
        );
    }

    true
}
