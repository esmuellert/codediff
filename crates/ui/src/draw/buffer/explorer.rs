//! Drawing the list of changed files.
//!
//! One row per line: `render::list` says what each row's pieces are and
//! what colour, `render::fit` says which of them survive the width, and this
//! places what is left. Nothing here decides any of the three.

use ratatui::buffer::Buffer as Cells;
use ratatui::layout::Rect;
use ratatui::style::Style;

use explorer::Row;

use crate::render::{cells, fit, list};
use crate::theme::Theme;
use crate::view::Viewport;
use crate::view::buffer::Explorer;

/// Draws the list into `area`.
///
/// Returns `false` if the pane is too narrow to say anything, which the caller
/// reports rather than filling with cut-off fragments.
pub fn draw(
    cells: &mut Cells,
    area: Rect,
    explorer: &Explorer,
    viewport: &Viewport,
    theme: &Theme,
    focused: bool,
) -> bool {
    if area.width < 4 || area.height == 0 {
        return false;
    }
    let rows = explorer.rows();
    let visible = viewport.visible(rows.len() as u32);
    for (offset, y) in (area.y..area.bottom()).enumerate() {
        let line = Rect {
            y,
            height: 1,
            ..area
        };
        let index = visible.start as usize + offset;
        let selected = focused && index as u32 == viewport.cursor();
        let background = if selected {
            theme.cursor_line
        } else {
            theme.normal
        };
        cells::fill(cells, line, background);
        if let Some(row) = rows.get(index) {
            paint(cells, line, row, theme, background);
        }
    }
    true
}

/// Writes one row's surviving pieces across a line.
fn paint(cells: &mut Cells, line: Rect, row: &Row, theme: &Theme, background: Style) {
    let (left, right) = list::pieces(row, theme, background);
    let fitted = fit::fit(&left, &right, line.width as usize);
    let mut x = 0;
    for piece in &fitted.left {
        x = cells::write(cells, line, x, &piece.text, piece.style);
    }
    x += fitted.gap as u16;
    for piece in &fitted.right {
        x = cells::write(cells, line, x, &piece.text, piece.style);
    }
}
