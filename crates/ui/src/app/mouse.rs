//! Mouse event handling: scroll, click, drag, and hit-testing.

use crossterm::event::{MouseButton, MouseEvent, MouseEventKind};

use crate::state::BufferType;
use crate::state::selection::{Pos, Selection};

use super::{Flow, PendingSelection, Session};

impl Session {
    /// Handles a mouse event — scroll, click, or drag.
    pub(super) fn handle_mouse(&mut self, mouse: &MouseEvent) -> Flow {
        let map = self.screen_map.borrow().clone();
        let mut view = self.view.borrow_mut();

        match mouse.kind {
            MouseEventKind::ScrollUp | MouseEventKind::ScrollDown => {
                let delta: i32 = if mouse.kind == MouseEventKind::ScrollUp {
                    -3
                } else {
                    3
                };
                if let Some((pane_id, _)) = map.pane_at(mouse.column, mouse.row) {
                    let pane = view.tab().pane(pane_id);
                    let view_lines = view.buffer(pane.buffer).view_lines();
                    view.tab_mut()
                        .pane_mut(pane_id)
                        .viewport
                        .scroll(delta, view_lines);
                }
            }
            MouseEventKind::Down(MouseButton::Left) => {
                // Clear any existing selection — a new click always resets.
                view.selection = None;
                self.pending_selection = None;

                if let Some(ta) = map.text_area_at(mouse.column, mouse.row) {
                    let pane_id = ta.pane;
                    let column = ta.column;
                    view.tab_mut().set_focus(pane_id);
                    let pos = ta.to_pos(mouse.column, mouse.row, &view.tab().pane(pane_id).viewport);
                    let buf_id = view.tab().pane(pane_id).buffer;
                    let view_lines = view.buffer(buf_id).view_lines();
                    if pos.line < view_lines {
                        // Record the anchor — a selection starts only on drag.
                        self.pending_selection = Some(PendingSelection {
                            pane: pane_id,
                            column,
                            anchor: pos,
                        });
                        view.tab_mut()
                            .pane_mut(pane_id)
                            .viewport
                            .place(pos.line, view_lines);
                    }
                } else if let Some((pane_id, area)) = map.pane_at(mouse.column, mouse.row) {
                    view.tab_mut().set_focus(pane_id);
                    let line_in_pane = (mouse.row - area.y) as u32;
                    let buf_id = view.tab().pane(pane_id).buffer;
                    let view_lines = view.buffer(buf_id).view_lines();
                    {
                        let viewport = &mut view.tab_mut().pane_mut(pane_id).viewport;
                        let target = viewport.top() + line_in_pane;
                        if target >= view_lines {
                            return Flow::Continue;
                        }
                        viewport.place(target, view_lines);
                    }
                    if matches!(view.buffer(buf_id).buffer_type(), BufferType::Explorer(_)) {
                        let selected = view.selected_file().cloned();
                        drop(view);
                        self.selected = selected;
                        return Flow::Continue;
                    }
                }
            }
            MouseEventKind::Drag(MouseButton::Left) => {
                // Promote pending to a real selection on first drag.
                if let Some(pending) = self.pending_selection.take() {
                    view.selection =
                        Some((pending.pane, Selection::start(pending.column, pending.anchor)));
                }
                if let Some((pane_id, sel)) = view.selection
                    && let Some(ta) = map.text_area_of(pane_id, sel.column)
                {
                    let pos = ta.to_pos(mouse.column, mouse.row, &view.tab().pane(pane_id).viewport);
                    let buf_id = view.tab().pane(pane_id).buffer;
                    let view_lines = view.buffer(buf_id).view_lines();
                    let pos = Pos::new(pos.line.min(view_lines.saturating_sub(1)), pos.col);
                    view.selection.as_mut().expect("just set").1.update(pos);
                }
            }
            MouseEventKind::Up(MouseButton::Left) => {
                // A click without a drag makes no selection.
                self.pending_selection = None;
                // A drag that returned to its anchor leaves nothing selected.
                if view.selection.as_ref().is_some_and(|(_, sel)| sel.is_empty()) {
                    view.selection = None;
                }
            }
            _ => {}
        }
        Flow::Continue
    }
}
