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
use crate::render::gutter;
use crate::render::layout::gutter_width;
use crate::theme::Theme;
use crate::view::Viewport;
use crate::view::buffer::SingleFile;

pub fn draw(
    buf: &mut Buffer,
    area: Rect,
    data: &SingleFile,
    view: &Viewport,
    theme: &Theme,
) -> bool {
    let width = gutter_width(data.rows());
    if area.width < width + 4 || area.height == 0 {
        return false;
    }
    let text = Rect {
        x: area.x + width,
        width: area.width - width,
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
        gutter::draw(
            buf,
            Rect {
                y,
                height: 1,
                width,
                ..area
            },
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
