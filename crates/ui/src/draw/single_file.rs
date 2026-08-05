//! Drawing one file into a pane.
//!
//! No second side, so no alignment, no filler and no divider — one column of
//! numbered lines. Not a diff with something switched off: a different buffer
//! kind, which is why nothing here has a branch for the missing side.

use align::DiffVersion;
use line_index::DEFAULT_TAB_WIDTH;
use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::Style;

use crate::paint::Spans;
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
    syntax: bool,
) -> bool {
    let width = gutter_width(data.lines());
    if area.width < width + 4 || area.height == 0 {
        return false;
    }
    let text = Rect {
        x: area.x + width,
        width: area.width - width,
        ..area
    };

    let spans = if syntax { data.spans() } else { Spans::Off };
    let visible = view.visible(data.lines());
    for (offset, line) in visible.clone().enumerate() {
        let y = area.y + offset as u16;
        let base = theme.normal.patch(if line == view.cursor() {
            theme.cursor_line
        } else {
            Style::new()
        });
        let numbers = base.patch(if line == view.cursor() {
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
            line + 1,
            numbers,
        );
        cells::paint(
            buf,
            Rect {
                y,
                height: 1,
                ..text
            },
            data.line(line).unwrap_or(""),
            DEFAULT_TAB_WIDTH,
            view.left(),
            Ink {
                base,
                emphasis: base,
                spans: &[],
                // The gutter shows `line + 1`, and so does this.
                syntax: spans.line(DiffVersion::Modified, line + 1),
                code: &theme.code,
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
