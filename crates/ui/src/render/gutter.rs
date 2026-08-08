//! One line number, drawn in its own file so the two call sites share one
//! definition.

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
