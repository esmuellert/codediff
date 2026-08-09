//! Mouse event handling: scroll, click, and hit-testing.

use crossterm::event::{MouseButton, MouseEvent, MouseEventKind};
use ratatui::layout::Rect;

use crate::render::layout;
use crate::view::{BufferType, Layout, PaneId};

use super::{Flow, HitMap, Session};

impl HitMap {
    /// Returns the pane at a screen position, if any.
    fn hit_test(&self, col: u16, row: u16) -> Option<(PaneId, Rect)> {
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
}

impl Session {
    /// Handles a mouse event — scroll or click.
    pub(super) fn handle_mouse(&mut self, mouse: &MouseEvent) -> Flow {
        match mouse.kind {
            MouseEventKind::ScrollUp | MouseEventKind::ScrollDown => {
                let delta: i32 = if mouse.kind == MouseEventKind::ScrollUp {
                    -3
                } else {
                    3
                };
                if let Some((pane_id, _)) = self.hit_map.hit_test(mouse.column, mouse.row) {
                    let pane = self.view.tab().pane(pane_id);
                    let view_lines = self.view.buffer(pane.buffer).view_lines();
                    self.view
                        .tab_mut()
                        .pane_mut(pane_id)
                        .viewport
                        .scroll(delta, view_lines);
                }
            }
            MouseEventKind::Down(MouseButton::Left) => {
                if let Some((pane_id, area)) = self.hit_map.hit_test(mouse.column, mouse.row) {
                    self.view.tab_mut().set_focus(pane_id);
                    let line_in_pane = (mouse.row - area.y) as u32;
                    let (buffer, viewport) = self.view.focused_mut();
                    let target = viewport.top() + line_in_pane;
                    if target >= buffer.view_lines() {
                        return Flow::Continue;
                    }
                    viewport.place(target, buffer.view_lines());
                    if matches!(buffer.buffer_type(), BufferType::Explorer(_)) {
                        self.open();
                    }
                }
            }
            _ => {}
        }
        Flow::Continue
    }

    /// Records where each pane landed, so a click can say which one it hit.
    pub(super) fn update_hit_map(&mut self, area: Rect) {
        self.hit_map.panes.clear();
        let Some((body, _status)) = layout::screen(area) else {
            self.hit_map.body = Rect::default();
            return;
        };
        self.hit_map.body = body;

        let places = match self.view.tab().layout() {
            Layout::Split { left } => layout::split(body, left),
            Layout::Full => None,
        };
        let panes: Vec<PaneId> = self.view.tab().ids().collect();
        match places {
            Some((left_area, _border, right_area)) => {
                if let Some(&id) = panes.first() {
                    self.hit_map.panes.push((id, left_area));
                }
                if let Some(&id) = panes.get(1) {
                    self.hit_map.panes.push((id, right_area));
                }
            }
            None => {
                if let Some(&id) = panes.first() {
                    self.hit_map.panes.push((id, body));
                }
            }
        }
    }
}
