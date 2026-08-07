//! Drawing the list of changed files.
//!
//! One row per line, each fitted to the pane by `render::explorer` and then
//! coloured a region at a time. Nothing here decides what is in a row or what
//! survives a narrow pane: this places the surviving pieces and picks their
//! styles, which is all a drawing step should do.

use ratatui::buffer::Buffer as Cells;
use ratatui::layout::Rect;
use ratatui::style::{Modifier, Style};

use explorer::{Region, RegionType, Row};
use file_types::ChangeType;

use crate::render::{cells, explorer as fit};
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

/// Writes one row's surviving regions across a line.
fn paint(cells: &mut Cells, line: Rect, row: &Row, theme: &Theme, background: Style) {
    let fitted = fit::fit(&row.left, &row.right, line.width as usize);
    let mut x = 0;
    for region in &fitted.left {
        x = cells::write(
            cells,
            line,
            x,
            &region.text,
            style(region, theme, background),
        );
    }
    x += fitted.gap as u16;
    for region in &fitted.right {
        x = cells::write(
            cells,
            line,
            x,
            &region.text,
            style(region, theme, background),
        );
    }
}

/// What one piece of a row looks like.
///
/// A colour from the theme's list table, over the row's own background rather
/// than replacing it, so the selected row stays visibly selected under every
/// colour it holds.
///
/// Bold is applied here rather than stored in the theme: a heading is bold in
/// every theme, so it is structural. That is the same division `Code` makes,
/// where weight comes from the scope table and colour from the theme.
fn style(region: &Region, theme: &Theme, background: Style) -> Style {
    let list = &theme.list;
    let colour = match region.region_type {
        RegionType::Heading => list.heading,
        RegionType::Marker | RegionType::Fold => list.marker,
        RegionType::Directory => list.directory,
        RegionType::Name => list.name,
        RegionType::Moved => list.moved,
        RegionType::Count => list.count,
        RegionType::Added => list.added,
        RegionType::Removed => list.removed,
        RegionType::Spacer => list.name,
        RegionType::Status(change) => match change {
            ChangeType::Added => list.new_file,
            ChangeType::Modified => list.modified,
            ChangeType::Deleted => list.deleted,
            ChangeType::Moved => list.renamed,
            ChangeType::Untracked => list.untracked,
            ChangeType::Conflicted => list.conflicted,
        },
    };
    let weight = match region.region_type {
        RegionType::Heading | RegionType::Status(_) => Modifier::BOLD,
        _ => Modifier::empty(),
    };
    background.fg(colour).add_modifier(weight)
}
