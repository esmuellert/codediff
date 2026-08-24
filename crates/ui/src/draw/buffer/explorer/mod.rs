//! Drawing the list of changed files.
//!
//! ```text
//! mod.rs        which lines are on screen, and what precedes each
//! tree.rs       indent guides and fold arrows
//! view_line.rs  one line: text, colour, placement
//! ```

mod tree;
mod view_line;

use ratatui::buffer::Buffer as Cells;
use ratatui::layout::Rect;

use crate::cells;
use crate::theme::Theme;
use crate::state::Viewport;
use crate::state::buffer::Explorer;

use crate::draw::TextRects;

/// Draws the list into `area`.
///
/// Returns `None` if the pane is too narrow to say anything.
pub fn draw(
    cells: &mut Cells,
    area: Rect,
    explorer: &Explorer,
    viewport: &Viewport,
    theme: &Theme,
    _focused: bool,
) -> Option<TextRects> {
    if area.width < 4 || area.height == 0 {
        return None;
    }
    let visible = viewport.visible(explorer.view_lines());
    for (offset, y) in (area.y..area.bottom()).enumerate() {
        let line = Rect {
            y,
            height: 1,
            ..area
        };
        let index = visible.start + offset as u32;
        let selected = index == viewport.cursor();
        let background = if selected {
            theme.cursor_line
        } else {
            theme.normal
        };
        cells::fill(cells, line, background);

        let Some(view_line) = explorer.view_line(index) else {
            continue;
        };
        // Guides come from the arrangement that has ancestors, and only from
        // it: a heading has no indent to describe, and a flat list has none to
        // read.
        let prefix = match explorer.nested_at(index) {
            Some((model, id)) => tree::prefix(model, id, theme, background),
            None => Vec::new(),
        };
        view_line::draw(cells, line, &view_line, prefix, theme, background);
    }
    Some(vec![])
}
