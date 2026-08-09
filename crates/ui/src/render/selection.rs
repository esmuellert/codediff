//! Painting a selection highlight over already-drawn cells.

use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::Style;

/// A position in view-line and cell-column coordinates.
#[derive(Debug, Clone, Copy)]
pub struct Range {
    pub start_line: u32,
    pub start_col: u32,
    pub end_line: u32,
    pub end_col: u32,
}

/// Paints the selection highlight over cells in a text area.
pub fn overlay(buf: &mut Buffer, rect: Rect, top: u32, left: u32, range: Range, style: Style) {
    for y in rect.y..rect.bottom() {
        let view_line = top + (y - rect.y) as u32;
        if view_line < range.start_line || view_line > range.end_line {
            continue;
        }
        for x in rect.x..rect.right() {
            let cell_col = left + (x - rect.x) as u32;
            let inside = if range.start_line == range.end_line {
                cell_col >= range.start_col && cell_col <= range.end_col
            } else if view_line == range.start_line {
                cell_col >= range.start_col
            } else if view_line == range.end_line {
                cell_col <= range.end_col
            } else {
                true
            };
            if inside && let Some(cell) = buf.cell_mut((x, y)) {
                cell.set_style(style);
            }
        }
    }
}
