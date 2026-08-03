//! Drawing one column of a side-by-side diff.
//!
//! The two columns are drawn by the same function from the **same slice of
//! rows**, differing only in which slot of each row they read. There is no
//! path by which one side could show row 40 while the other shows row 41.

use align::{Alignment, DiffVersion, Row, RowKind, Slot};
use line_index::DEFAULT_TAB_WIDTH;
use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::Style;

use crate::render::cells::{self, Ink};
use crate::render::gutter;
use crate::render::layout::Column;
use crate::theme::Theme;

/// The hatching drawn where a side has no line.
///
/// The plugin's choice, and a good one: it is visibly not content, and it
/// slants, so a block of them reads as a gap rather than as a border.
const FILLER: &str = "╱";

/// What every row of a frame has in common.
///
/// Gathered into one value because these travel together through the whole
/// call chain, and passing them individually made every function here take
/// nine arguments in an order nothing checked.
#[derive(Clone, Copy)]
pub struct Painter<'a> {
    pub alignment: &'a Alignment,
    pub theme: &'a Theme,
    /// Index of the first row on screen.
    pub top: u32,
    /// Index of the row the cursor is on.
    pub cursor: u32,
    /// Horizontal scroll, in cells.
    pub left: u32,
}

/// Draws the visible rows of one column.
///
/// `rows` are the rows starting at `painter.top`.
pub fn draw(
    buf: &mut Buffer,
    column: Column,
    version: DiffVersion,
    rows: &[Row],
    painter: Painter<'_>,
) {
    for (offset, row) in rows.iter().enumerate() {
        let y = column.text.y + offset as u16;
        if y >= column.text.bottom() {
            break;
        }
        line(
            buf,
            column,
            y,
            version,
            row,
            painter.top + offset as u32,
            painter,
        );
    }

    // Below the end of the document. Neovim draws `~`; we leave it blank,
    // because a reviewer wants the two sides' ends to be visually comparable
    // and a column of tildes on one side only would read as content.
    let drawn = rows.len() as u16;
    for y in (column.text.y + drawn)..column.text.bottom() {
        cells::fill(buf, column.row(y), painter.theme.normal);
    }
}

fn line(
    buf: &mut Buffer,
    column: Column,
    y: u16,
    version: DiffVersion,
    row: &Row,
    index: u32,
    painter: Painter<'_>,
) {
    let theme = painter.theme;
    let slot = match version {
        DiffVersion::Original => row.original,
        DiffVersion::Modified => row.modified,
    };

    let Slot::Line(number) = slot else {
        // No line on this side at all. Hatch the whole width, gutter included,
        // so the gap is unmistakable and no line number is implied.
        cells::hatch(buf, column.row(y), FILLER, theme.normal.patch(theme.filler));
        return;
    };

    let is_cursor = index == painter.cursor;
    // The row's own style, which everything on the row is then patched over.
    let base = theme
        .normal
        .patch(role(row.kind, version, number, is_cursor, painter));

    let number_style = base.patch(if is_cursor {
        theme.line_number_current
    } else {
        theme.line_number
    });
    gutter::draw(
        buf,
        Rect {
            y,
            height: 1,
            ..column.gutter
        },
        number,
        number_style,
    );

    let emphasis = base.patch(match (row.kind, version) {
        (RowKind::Unchanged, _) => Style::new(),
        (_, DiffVersion::Original) => theme.deleted_text,
        (_, DiffVersion::Modified) => theme.inserted_text,
    });

    let spans: Vec<_> = painter
        .alignment
        .spans(version, number)
        .into_iter()
        .map(|s| s.bytes)
        .collect();

    cells::paint(
        buf,
        Rect {
            y,
            height: 1,
            ..column.text
        },
        painter.alignment.line(version, number).unwrap_or(""),
        DEFAULT_TAB_WIDTH,
        painter.left,
        Ink {
            base,
            emphasis,
            spans: &spans,
        },
    );
}

/// What this row is, in theme terms, highest priority first.
///
/// A change outranks the cursor line: losing sight of which lines differ is
/// worse than losing sight of where the cursor is, and the line number still
/// says where the cursor is. `Style::new()` means "leave it as `normal`".
fn role(
    kind: RowKind,
    version: DiffVersion,
    number: u32,
    is_cursor: bool,
    painter: Painter<'_>,
) -> Style {
    let theme = painter.theme;
    if kind != RowKind::Unchanged {
        return match version {
            DiffVersion::Original => theme.deleted,
            DiffVersion::Modified => theme.inserted,
        };
    }
    if painter.alignment.moved(version, number).is_some() {
        return theme.moved;
    }
    if is_cursor {
        return theme.cursor_line;
    }
    Style::new()
}
