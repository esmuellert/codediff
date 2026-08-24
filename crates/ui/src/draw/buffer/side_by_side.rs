//! Drawing a side-by-side diff into one pane.
//!
//! Reads the [`Alignment`] the buffer already holds. Nothing here builds one:
//! the pipeline did that once, when the file was opened, and a frame that
//! rebuilt it would be redoing work whose inputs cannot have changed.
//!
//! [`Alignment`]: align::Alignment

use align::DiffVersion;
use file_types::DiffType;
use ratatui::buffer::Buffer as Cells;
use ratatui::layout::Rect;

use crate::draw::{Look, TextRects};
use crate::render::layout;
use crate::render::line::Painter;
use crate::cells;
use crate::render::column;
use crate::state::Viewport;
use crate::state::buffer::Buffer;
use crate::state::buffer::SideBySide;
use crate::state::selection::SelectionColumn;
use syntax::Spans;

/// Draws one diff into the pane's area.
///
/// Returns `None` if the pane is too narrow to draw, or the text rects drawn.
pub fn draw(
    buf: &mut Cells,
    area: Rect,
    buffer: &Buffer,
    data: &SideBySide,
    view: &Viewport,
    look: Look<'_>,
) -> Option<TextRects> {
    let Look { theme, syntax, .. } = look;
    let alignment = data.alignment();
    let frame = layout::columns(
        area,
        data.divider(),
        alignment.lines(DiffVersion::Original).len() as u32,
        alignment.lines(DiffVersion::Modified).len() as u32,
    )?;

    let visible = view.visible(buffer.view_lines());

    // Collected once and handed to both columns. Two columns reading one slice
    // cannot disagree about what line they are on.
    let lines: Vec<_> = alignment
        .view_lines_from(DiffType::SideBySide, visible.start)
        .take(visible.len())
        .collect();

    let painter = Painter {
        alignment,
        theme,
        syntax: if syntax {
            data.spans(look.store)
        } else {
            Spans::Off
        },
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
        cells::fill_repeat_pattern(
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

    Some(vec![
        (SelectionColumn::Original, frame.original.text),
        (SelectionColumn::Modified, frame.modified.text),
    ])
}
