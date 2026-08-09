//! Mouse text selection within one column.

/// Which text column within a pane the selection lives in.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SelectionColumn {
    /// The only column (single file, inline).
    Only,
    /// Left column of a side-by-side diff.
    Original,
    /// Right column of a side-by-side diff.
    Modified,
}

/// A position in buffer-local coordinates.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Pos {
    /// View line (absolute, not relative to viewport).
    pub line: u32,
    /// Cell column within the text area, accounting for horizontal scroll.
    pub col: u32,
}

impl Pos {
    pub fn new(line: u32, col: u32) -> Self {
        Self { line, col }
    }
}

impl PartialOrd for Pos {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for Pos {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        self.line.cmp(&other.line).then(self.col.cmp(&other.col))
    }
}

/// A text range within one column. Who owns it is stored alongside.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Selection {
    /// Which column the selection is confined to.
    pub column: SelectionColumn,
    /// Where the mouse was first pressed.
    pub anchor: Pos,
    /// Where the mouse is now (or was released).
    pub cursor: Pos,
}

impl Selection {
    pub fn start(column: SelectionColumn, pos: Pos) -> Self {
        Self {
            column,
            anchor: pos,
            cursor: pos,
        }
    }

    pub fn update(&mut self, pos: Pos) {
        self.cursor = pos;
    }

    pub fn start_pos(&self) -> Pos {
        self.anchor.min(self.cursor)
    }

    pub fn end_pos(&self) -> Pos {
        self.anchor.max(self.cursor)
    }

    /// Whether a given (view_line, cell_col) falls within the selection.
    pub fn contains(&self, line: u32, col: u32) -> bool {
        let start = self.start_pos();
        let end = self.end_pos();
        let pos = Pos::new(line, col);
        if pos < start || pos > end {
            return false;
        }
        if start.line == end.line {
            return col >= start.col && col <= end.col;
        }
        if line == start.line {
            return col >= start.col;
        }
        if line == end.line {
            return col <= end.col;
        }
        true
    }

    pub fn is_empty(&self) -> bool {
        self.anchor == self.cursor
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn single_line_selection() {
        let sel = Selection {
            column: SelectionColumn::Only,
            anchor: Pos::new(5, 3),
            cursor: Pos::new(5, 10),
        };
        assert!(sel.contains(5, 3));
        assert!(sel.contains(5, 7));
        assert!(sel.contains(5, 10));
        assert!(!sel.contains(5, 2));
        assert!(!sel.contains(5, 11));
        assert!(!sel.contains(4, 5));
        assert!(!sel.contains(6, 5));
    }

    #[test]
    fn multi_line_selection() {
        let sel = Selection {
            column: SelectionColumn::Original,
            anchor: Pos::new(2, 5),
            cursor: Pos::new(4, 8),
        };
        assert!(!sel.contains(2, 4));
        assert!(sel.contains(2, 5));
        assert!(sel.contains(2, 100));
        assert!(sel.contains(3, 0));
        assert!(sel.contains(3, 999));
        assert!(sel.contains(4, 0));
        assert!(sel.contains(4, 8));
        assert!(!sel.contains(4, 9));
    }

    #[test]
    fn reversed_anchor_and_cursor() {
        let sel = Selection {
            column: SelectionColumn::Modified,
            anchor: Pos::new(10, 20),
            cursor: Pos::new(8, 5),
        };
        assert!(sel.contains(8, 5));
        assert!(sel.contains(9, 0));
        assert!(sel.contains(10, 20));
        assert!(!sel.contains(10, 21));
        assert!(!sel.contains(8, 4));
    }

    #[test]
    fn empty_selection() {
        let sel = Selection::start(SelectionColumn::Only, Pos::new(3, 7));
        assert!(sel.is_empty());
        assert!(sel.contains(3, 7));
    }
}
