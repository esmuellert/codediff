//! How one line of a diff is coloured and drawn, wherever it lands.
//!
//! Both layouts draw the same thing at bottom: a line of one version, in the
//! style its line earns, with the characters that changed picked out. Side by
//! side puts it in one of two columns; inline puts it in the only one. Sharing
//! this is what stops the two drifting into different colours for the same
//! line.
//!
//! What is *not* here is where it goes — the rectangles are the layout's
//! business, and are passed in.

use align::{Alignment, DiffVersion, ViewLineType};
use line_index::DEFAULT_TAB_WIDTH;
use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::Style;

use crate::highlight::Spans;
use crate::render::cells::{self, Ink};
use crate::theme::Theme;

/// What every line of a frame has in common.
///
/// Gathered into one value because these travel together through the whole
/// call chain, and passing them individually made every function here take
/// nine arguments in an order nothing checked.
#[derive(Clone, Copy)]
pub struct Painter<'a> {
    pub alignment: &'a Alignment,
    pub theme: &'a Theme,
    /// What the language says about each line, as far as it has been read.
    ///
    /// Borrowed, and read-only: colouring more of the file is a decision made
    /// before the frame, because drawing holds no state.
    pub syntax: Spans<'a>,
    /// Index of the first line on screen.
    pub top: u32,
    /// Index of the line the cursor is on.
    pub cursor: u32,
    /// Horizontal scroll, in cells.
    pub left: u32,
}

/// The style the whole line wears, which everything on it is patched over.
pub fn base(
    kind: ViewLineType,
    version: DiffVersion,
    number: u32,
    is_cursor: bool,
    painter: Painter<'_>,
) -> Style {
    painter
        .theme
        .normal
        .patch(role(kind, version, number, is_cursor, painter))
}

/// What this line is, in theme terms, highest priority first.
///
/// A change outranks the cursor line: losing sight of which lines differ is
/// worse than losing sight of where the cursor is, and the line number still
/// says where the cursor is. `Style::new()` means "leave it as `normal`".
fn role(
    kind: ViewLineType,
    version: DiffVersion,
    number: u32,
    is_cursor: bool,
    painter: Painter<'_>,
) -> Style {
    let theme = painter.theme;
    if kind != ViewLineType::Unchanged {
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

/// How a line number is drawn on a line wearing `base`.
pub fn numbers(base: Style, is_cursor: bool, theme: &Theme) -> Style {
    base.patch(if is_cursor {
        theme.line_number_current
    } else {
        theme.line_number
    })
}

/// Draws one version's line, with the characters that changed picked out.
pub fn text(
    buf: &mut Buffer,
    area: Rect,
    version: DiffVersion,
    number: u32,
    kind: ViewLineType,
    base: Style,
    painter: Painter<'_>,
) {
    let theme = painter.theme;
    let emphasis = base.patch(match (kind, version) {
        (ViewLineType::Unchanged, _) => Style::new(),
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
        area,
        painter.alignment.line(version, number).unwrap_or(""),
        DEFAULT_TAB_WIDTH,
        painter.left,
        Ink {
            base,
            emphasis,
            spans: &spans,
            syntax: painter.syntax.line(version, number),
            code: &theme.code,
        },
    );
}
