//! Drawing a side-by-side diff into one pane.
//!
//! Reads the [`Alignment`] the buffer already holds. Nothing here builds one:
//! the pipeline did that once, when the file was opened, and a frame that
//! rebuilt it would be redoing work whose inputs cannot have changed.
//!
//! [`Alignment`]: align::Alignment

use align::Side;
use ratatui::buffer::Buffer;
use ratatui::layout::Rect;

use crate::render::layout;
use crate::render::{cells, column};
use crate::theme::Theme;
use crate::view::Viewport;
use crate::view::buffer::SideBySide;

/// Draws one diff into the pane's area.
///
/// Returns `false` if the pane is too narrow to draw, which the caller shows
/// as a message rather than a corrupt frame.
pub fn draw(
    buf: &mut Buffer,
    area: Rect,
    data: &SideBySide,
    view: &Viewport,
    theme: &Theme,
) -> bool {
    let alignment = data.alignment();
    let Some(frame) = layout::columns(
        area,
        data.divider(),
        alignment.lines(Side::Original).len() as u32,
        alignment.lines(Side::Modified).len() as u32,
    ) else {
        return false;
    };

    let visible = view.visible(data.rows());

    // Collected once and handed to both columns. Two columns reading one slice
    // cannot disagree about what row they are on.
    let rows: Vec<_> = alignment
        .rows_from(visible.start)
        .take(visible.len())
        .collect();

    let painter = column::Painter {
        alignment,
        theme,
        top: visible.start,
        cursor: view.cursor(),
        left: view.left(),
    };
    for (side, column) in frame.columns() {
        column::draw(buf, column, side, &rows, painter);
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
