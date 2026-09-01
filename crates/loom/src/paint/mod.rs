//! Ink on cells.

mod host;

pub use host::{
    Canvas, CanvasProps, Column, ColumnProps, Divider, DividerProps, Gap, GapProps, Row, RowProps,
    Stack, StackProps, Text, TextProps,
};

use ratatui::buffer::Buffer as Cells;
use ratatui::layout::Rect;

/// What a `Canvas` is handed.
pub struct Paint<'a> {
    cells: &'a mut Cells,
    area: Rect,
    clip: Rect,
    focused: bool,
}

impl<'a> Paint<'a> {
    pub(crate) fn new(cells: &'a mut Cells, area: Rect, clip: Rect, focused: bool) -> Self {
        Self {
            cells,
            area,
            clip,
            focused,
        }
    }

    /// The cell grid.
    pub fn cells(&mut self) -> &mut Cells {
        self.cells
    }
    /// This node's rectangle.
    pub fn area(&self) -> Rect {
        self.area
    }
    /// `area` intersected with every clipping ancestor.
    pub fn clip(&self) -> Rect {
        self.clip
    }
    /// Whether this node holds focus.
    pub fn has_focus(&self) -> bool {
        self.focused
    }

    /// Writes one cell, when it lies inside the clip. I4 is what this keeps.
    pub fn set(&mut self, x: u16, y: u16, symbol: &str, style: ratatui::style::Style) {
        if !self.clip.contains(ratatui::layout::Position { x, y }) {
            return;
        }
        if let Some(cell) = self.cells.cell_mut((x, y)) {
            cell.set_symbol(symbol);
            cell.set_style(style);
        }
    }

    /// Writes a string from `x`, stopping at the clip's right edge. Answers
    /// how many cells it took.
    pub fn write(&mut self, x: u16, y: u16, text: &str, style: ratatui::style::Style) -> u16 {
        use ratatui::text::Span;
        let mut at = x;
        for grapheme in unicode_graphemes(text) {
            let width = Span::raw(grapheme).width() as u16;
            if width == 0 {
                continue;
            }
            if at.saturating_add(width) > self.clip.right() {
                break;
            }
            self.set(at, y, grapheme, style);
            // A wide glyph leaves the cell to its right blank.
            for over in 1..width {
                self.set(at + over, y, "", style);
            }
            at = at.saturating_add(width);
        }
        at.saturating_sub(x)
    }
}

/// Graphemes, without pulling in a segmentation crate: ratatui's own `Span`
/// measures, and `char_indices` is the boundary set this program's text needs.
fn unicode_graphemes(text: &str) -> impl Iterator<Item = &str> {
    text.char_indices()
        .map(move |(at, ch)| &text[at..at + ch.len_utf8()])
}
