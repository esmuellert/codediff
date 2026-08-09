//! Drawing one column of a side-by-side diff.
//!
//! The two columns are drawn by the same function from the **same slice of
//! lines**, differing only in which slot of each line they read. There is no
//! path by which one side could show line 40 while the other shows line 41.

use align::{DiffVersion, Slot, ViewLine};
use ratatui::buffer::Buffer;
use ratatui::layout::Rect;

use crate::render::cells;
use crate::render::gutter;
use crate::render::layout::Column;
use crate::render::line::{self, Painter};

/// The symbol repeated where a side has no line.
///
/// The plugin's choice, and a good one: it is visibly not content, and it
/// slants, so a block of them reads as a gap rather than as a border.
const FILLER: &str = "╱";

/// Draws the visible lines of one column.
///
/// `lines` are the view lines starting at `painter.top`.
pub fn draw(
    buf: &mut Buffer,
    column: Column,
    version: DiffVersion,
    lines: &[ViewLine],
    painter: Painter<'_>,
) {
    for (offset, line) in lines.iter().enumerate() {
        let y = column.text.y + offset as u16;
        if y >= column.text.bottom() {
            break;
        }
        view_line(
            buf,
            column,
            y,
            version,
            line,
            painter.top + offset as u32,
            painter,
        );
    }

    // Below the end of the document. Neovim draws `~`; we leave it blank,
    // because a reviewer wants the two sides' ends to be visually comparable
    // and a column of tildes on one side only would read as content.
    let drawn = lines.len() as u16;
    for y in (column.text.y + drawn)..column.text.bottom() {
        cells::fill(buf, column.row(y), painter.theme.normal);
    }
}

fn view_line(
    buf: &mut Buffer,
    column: Column,
    y: u16,
    version: DiffVersion,
    line: &ViewLine,
    index: u32,
    painter: Painter<'_>,
) {
    let theme = painter.theme;
    let slot = match version {
        DiffVersion::Original => line.original,
        DiffVersion::Modified => line.modified,
    };

    let Slot::Line(number) = slot else {
        // No line on this side at all. Hatch the whole width, gutter included,
        // so the gap is unmistakable and no line number is implied.
        cells::fill_repeat_pattern(buf, column.row(y), FILLER, theme.normal.patch(theme.filler));
        return;
    };

    let is_cursor = index == painter.cursor;
    let base = line::base(line.kind, version, number, is_cursor, painter);
    gutter::draw(
        buf,
        Rect {
            y,
            height: 1,
            ..column.gutter
        },
        number,
        line::numbers(base, is_cursor, theme),
    );
    line::text(
        buf,
        Rect {
            y,
            height: 1,
            ..column.text
        },
        version,
        number,
        line.kind,
        base,
        painter,
    );
}
