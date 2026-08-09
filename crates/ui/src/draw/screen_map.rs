//! Where things landed on screen, so a mouse event can say what it hit.
//!
//! Filled by the draw pass (the one source of truth), read by the event loop.

use ratatui::layout::Rect;

use crate::view::PaneId;
use crate::view::Viewport;
use crate::view::selection::{Pos, SelectionColumn};

/// One text area within a pane, for per-column hit-testing and selection.
#[derive(Debug, Clone)]
pub struct TextArea {
    pub pane: PaneId,
    pub column: SelectionColumn,
    pub rect: Rect,
}

impl TextArea {
    /// Converts a terminal position to buffer-local coordinates.
    pub fn to_pos(&self, col: u16, row: u16, viewport: &Viewport) -> Pos {
        let row_offset = row.saturating_sub(self.rect.y) as u32;
        let col_offset = col.saturating_sub(self.rect.x) as u32;
        Pos::new(viewport.top() + row_offset, viewport.left() + col_offset)
    }
}

/// Where each pane and text area was drawn.
#[derive(Debug, Default, Clone)]
pub struct ScreenMap {
    pub panes: Vec<(PaneId, Rect)>,
    pub body: Rect,
    pub text_areas: Vec<TextArea>,
}

impl ScreenMap {
    pub fn clear(&mut self) {
        self.panes.clear();
        self.text_areas.clear();
        self.body = Rect::default();
    }

    /// Returns the pane at a screen position, if any.
    pub fn pane_at(&self, col: u16, row: u16) -> Option<(PaneId, Rect)> {
        self.panes
            .iter()
            .find(|(_, rect)| {
                col >= rect.x
                    && col < rect.x + rect.width
                    && row >= rect.y
                    && row < rect.y + rect.height
            })
            .map(|(id, rect)| (*id, *rect))
    }

    /// Returns the text area at a screen position, if any.
    pub fn text_area_at(&self, col: u16, row: u16) -> Option<&TextArea> {
        self.text_areas.iter().find(|ta| {
            col >= ta.rect.x
                && col < ta.rect.x + ta.rect.width
                && row >= ta.rect.y
                && row < ta.rect.y + ta.rect.height
        })
    }

    /// Returns the text area for a specific pane and column.
    pub fn text_area_of(&self, pane: PaneId, column: SelectionColumn) -> Option<&TextArea> {
        self.text_areas
            .iter()
            .find(|ta| ta.pane == pane && ta.column == column)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::view::PaneId;

    fn map_with_two_panes() -> ScreenMap {
        let mut m = ScreenMap::default();
        m.body = Rect::new(0, 0, 120, 29);
        m.panes.push((PaneId::new(0), Rect::new(0, 0, 40, 29)));
        m.panes.push((PaneId::new(1), Rect::new(41, 0, 79, 29)));
        m.text_areas.push(TextArea {
            pane: PaneId::new(1),
            column: SelectionColumn::Original,
            rect: Rect::new(45, 0, 34, 29),
        });
        m.text_areas.push(TextArea {
            pane: PaneId::new(1),
            column: SelectionColumn::Modified,
            rect: Rect::new(80, 0, 40, 29),
        });
        m
    }

    #[test]
    fn pane_at_finds_correct_pane() {
        let m = map_with_two_panes();
        assert_eq!(m.pane_at(10, 5).unwrap().0, PaneId::new(0));
        assert_eq!(m.pane_at(50, 5).unwrap().0, PaneId::new(1));
    }

    #[test]
    fn pane_at_misses_outside() {
        let m = map_with_two_panes();
        assert!(m.pane_at(40, 5).is_none());
    }

    #[test]
    fn pane_at_boundary_hit() {
        let m = map_with_two_panes();
        assert_eq!(m.pane_at(0, 0).unwrap().0, PaneId::new(0));
        assert_eq!(m.pane_at(39, 0).unwrap().0, PaneId::new(0));
        assert_eq!(m.pane_at(41, 0).unwrap().0, PaneId::new(1));
    }

    #[test]
    fn text_area_at_finds_column() {
        let m = map_with_two_panes();
        let ta = m.text_area_at(50, 10).unwrap();
        assert_eq!(ta.column, SelectionColumn::Original);

        let ta = m.text_area_at(90, 10).unwrap();
        assert_eq!(ta.column, SelectionColumn::Modified);
    }

    #[test]
    fn text_area_at_misses_gutter() {
        let m = map_with_two_panes();
        assert!(m.text_area_at(42, 5).is_none());
    }

    #[test]
    fn text_area_of_finds_by_pane_and_column() {
        let m = map_with_two_panes();
        let ta = m
            .text_area_of(PaneId::new(1), SelectionColumn::Modified)
            .unwrap();
        assert_eq!(ta.rect.x, 80);

        assert!(
            m.text_area_of(PaneId::new(0), SelectionColumn::Original)
                .is_none()
        );
    }

    #[test]
    fn to_pos_accounts_for_scroll() {
        let m = map_with_two_panes();
        let ta = m
            .text_area_of(PaneId::new(1), SelectionColumn::Modified)
            .unwrap();

        let mut vp = Viewport::new();
        vp.set_height(29, 100);
        vp.scroll(10, 100);
        use crate::input::Motion;
        vp.motion(Motion::ScrollRight, 1, 100);

        let pos = ta.to_pos(85, 3, &vp);
        assert_eq!(pos.line, vp.top() + 3);
        assert_eq!(pos.col, vp.left() + 5);
    }

    #[test]
    fn clear_empties_everything() {
        let mut m = map_with_two_panes();
        m.clear();
        assert!(m.panes.is_empty());
        assert!(m.text_areas.is_empty());
        assert_eq!(m.body, Rect::default());
    }
}
