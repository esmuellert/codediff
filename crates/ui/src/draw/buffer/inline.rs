//! Drawing a diff one version per line, into one pane.
//!
//! Two gutters and one text column. Every line belongs to one version, and the
//! missing number is what says which: no modified number means the line
//! was deleted, no original number means it was inserted. An unchanged line
//! carries both, since both versions have it there.
//!
//! No fillers and no divider: nothing is drawn opposite anything, so there is
//! no gap to fill. That is the whole visual difference from
//! [`side_by_side`](crate::draw::buffer::side_by_side) — the colours, the
//! inner-change highlighting and the cursor line are shared with it verbatim,
//! in [`line`](crate::render::line).

use align::{DiffVersion, Slot, ViewLine};
use file_types::DiffType;
use ratatui::buffer::Buffer as Cells;
use ratatui::layout::Rect;

use crate::draw::Look;
use crate::render::layout::{self, InlineFrame};
use crate::render::line::{self, Painter};
use crate::render::{cells, gutter};
use crate::syntax::Spans;
use crate::view::Viewport;
use crate::view::buffer::Buffer;
use crate::view::buffer::Inline;

/// Draws one diff into the pane's area.
///
/// Returns `false` if the pane is too narrow to draw, which the caller shows
/// as a message rather than a corrupt frame.
pub fn draw(
    buf: &mut Cells,
    area: Rect,
    buffer: &Buffer,
    data: &Inline,
    view: &Viewport,
    look: Look<'_>,
) -> bool {
    let Look { theme, syntax, .. } = look;
    let alignment = data.alignment();
    let Some(frame) = layout::inline(
        area,
        alignment.lines(DiffVersion::Original).len() as u32,
        alignment.lines(DiffVersion::Modified).len() as u32,
    ) else {
        return false;
    };

    let visible = view.visible(buffer.view_lines());
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

    let mut drawn = 0;
    for (offset, current) in alignment
        .view_lines_from(DiffType::Inline, visible.start)
        .take(visible.len())
        .enumerate()
    {
        let y = frame.text.y + offset as u16;
        if y >= frame.text.bottom() {
            break;
        }
        view_line(
            buf,
            &frame,
            y,
            &current,
            visible.start + offset as u32,
            painter,
        );
        drawn += 1;
    }

    // Below the end of the document. Blank rather than neovim's `~`, for the
    // reason the columns are: a marker only one layout draws would read as
    // content.
    for y in (frame.text.y + drawn)..frame.text.bottom() {
        cells::fill(buf, frame.row(y), theme.normal);
    }
    true
}

fn view_line(
    buf: &mut Cells,
    frame: &InlineFrame,
    y: u16,
    line: &ViewLine,
    index: u32,
    painter: Painter<'_>,
) {
    let theme = painter.theme;
    // Which version this line shows. Only an unchanged line has both, and then
    // the two lines are the same text, so either answers.
    let (version, number) = match (line.modified, line.original) {
        (Slot::Line(n), _) => (DiffVersion::Modified, n),
        (_, Slot::Line(n)) => (DiffVersion::Original, n),
        // A line showing neither version cannot occur inline — a change
        // contributes one line per line it has, never a line for a line it
        // lacks — but a blank line is a better answer than a panic.
        (Slot::Filler, Slot::Filler) => {
            cells::fill(buf, frame.row(y), theme.normal);
            return;
        }
    };

    let is_cursor = index == painter.cursor;
    let base = line::base(line.kind, version, number, is_cursor, painter);
    let numbers = line::numbers(base, is_cursor, theme);

    for (side, area) in frame.gutters() {
        let area = Rect {
            y,
            height: 1,
            ..area
        };
        match slot(line, side) {
            // Present in this version: its own number, which for an unchanged
            // line differs between the two.
            Slot::Line(n) => gutter::draw(buf, area, n, numbers),
            // Absent from this version, which is what says whether the line
            // was deleted or inserted. Blank, in the line's own colour, so the
            // change background runs edge to edge.
            Slot::Filler => cells::fill(buf, area, base),
        }
    }

    line::text(
        buf,
        Rect {
            y,
            height: 1,
            ..frame.text
        },
        version,
        number,
        line.kind,
        base,
        painter,
    );
}

fn slot(line: &ViewLine, version: DiffVersion) -> Slot {
    match version {
        DiffVersion::Original => line.original,
        DiffVersion::Modified => line.modified,
    }
}
