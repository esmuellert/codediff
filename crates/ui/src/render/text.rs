//! Drawing one file into a pane.
//!
//! No second side, so no alignment, no filler and no divider — one column of
//! numbered lines. Not a diff with something switched off: a different buffer
//! kind, which is why nothing here has a branch for the missing side.

use line_index::DEFAULT_TAB_WIDTH;
use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::Style;

use crate::render::cells::{self, Ink};
use crate::render::layout::gutter_width;
use crate::theme::Theme;
use crate::view::Viewport;
use crate::view::buffer::Text;

pub fn draw(buf: &mut Buffer, area: Rect, data: &Text, view: &Viewport, theme: &Theme) -> bool {
    let gutter = gutter_width(data.rows());
    if area.width < gutter + 4 || area.height == 0 {
        return false;
    }
    let text = Rect {
        x: area.x + gutter,
        width: area.width - gutter,
        ..area
    };

    let visible = view.visible(data.rows());
    for (offset, row) in visible.clone().enumerate() {
        let y = area.y + offset as u16;
        let base = theme.normal.patch(if row == view.cursor() {
            theme.cursor_line
        } else {
            Style::new()
        });
        let numbers = base.patch(if row == view.cursor() {
            theme.line_number_current
        } else {
            theme.line_number
        });
        number(
            buf,
            Rect {
                y,
                height: 1,
                ..area
            },
            gutter,
            row + 1,
            numbers,
        );
        cells::paint(
            buf,
            Rect {
                y,
                height: 1,
                ..text
            },
            data.line(row).unwrap_or(""),
            DEFAULT_TAB_WIDTH,
            view.left(),
            Ink {
                base,
                emphasis: base,
                spans: &[],
            },
        );
    }

    for y in (area.y + visible.len() as u16)..area.bottom() {
        cells::fill(
            buf,
            Rect {
                y,
                height: 1,
                ..area
            },
            theme.normal,
        );
    }
    true
}

/// The line number, right-aligned with one space before the text.
fn number(buf: &mut Buffer, row: Rect, width: u16, line: u32, style: Style) {
    let area = Rect { width, ..row };
    cells::fill(buf, area, style);
    let label = line.to_string();
    let offset = area
        .width
        .saturating_sub(1)
        .saturating_sub(label.chars().count() as u16);
    cells::write(buf, area, offset, &label, style);
}
