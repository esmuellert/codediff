//! One line number.
//!
//! Its own file because it is drawn in two places — beside a diff's column and
//! beside a single file — and two copies of "what a line number looks like"
//! can drift. They never appear on one screen, so nothing would compare them
//! and no test would catch it.

use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::Style;

use crate::render::cells;

/// Draws one line number, right-aligned with one space before the text.
///
/// `area` is the gutter's rectangle for a single row; its width is
/// [`layout::gutter_width`], which sizes itself to the file's longest number.
///
/// [`layout::gutter_width`]: crate::render::layout::gutter_width
pub fn draw(buf: &mut Buffer, area: Rect, number: u32, style: Style) {
    cells::fill(buf, area, style);
    let label = number.to_string();
    let offset = area
        .width
        .saturating_sub(1)
        .saturating_sub(label.chars().count() as u16);
    cells::write(buf, area, offset, &label, style);
}
