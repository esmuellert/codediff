//! Mouse event handling: scroll, click, drag, and hit-testing.

use crossterm::event::{MouseButton, MouseEvent, MouseEventKind};

use crate::view::BufferType;
use crate::view::selection::{Pos, Selection};

use super::{Flow, PendingSelection, Session};

impl Session {
    /// Handles a mouse event — scroll, click, or drag.
    pub(super) fn handle_mouse(&mut self, mouse: &MouseEvent) -> Flow {
        match mouse.kind {
            MouseEventKind::ScrollUp | MouseEventKind::ScrollDown => {
                let delta: i32 = if mouse.kind == MouseEventKind::ScrollUp {
                    -3
                } else {
                    3
                };
                if let Some((pane_id, _)) = self.screen_map.pane_at(mouse.column, mouse.row) {
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
                // Clear any existing selection — a new click always resets.
                self.view.selection = None;
                self.pending_selection = None;

                if let Some(ta) = self.screen_map.text_area_at(mouse.column, mouse.row) {
                    let pane_id = ta.pane;
                    let column = ta.column;
                    self.view.tab_mut().set_focus(pane_id);
                    let pos = ta.to_pos(
                        mouse.column,
                        mouse.row,
                        &self.view.tab().pane(pane_id).viewport,
                    );
                    let buf_id = self.view.tab().pane(pane_id).buffer;
                    let view_lines = self.view.buffer(buf_id).view_lines();
                    if pos.line < view_lines {
                        // Record anchor — selection starts only on drag.
                        self.pending_selection = Some(PendingSelection {
                            pane: pane_id,
                            column,
                            anchor: pos,
                        });
                        // Move cursor to the clicked line.
                        self.view
                            .tab_mut()
                            .pane_mut(pane_id)
                            .viewport
                            .place(pos.line, view_lines);
                    }
                } else if let Some((pane_id, area)) =
                    self.screen_map.pane_at(mouse.column, mouse.row)
                {
                    self.view.tab_mut().set_focus(pane_id);
                    let line_in_pane = (mouse.row - area.y) as u32;
                    let buf_id = self.view.tab().pane(pane_id).buffer;
                    let view_lines = self.view.buffer(buf_id).view_lines();
                    {
                        let viewport = &mut self.view.tab_mut().pane_mut(pane_id).viewport;
                        let target = viewport.top() + line_in_pane;
                        if target >= view_lines {
                            return Flow::Continue;
                        }
                        viewport.place(target, view_lines);
                    }
                    if matches!(
                        self.view.buffer(buf_id).buffer_type(),
                        BufferType::Explorer(_)
                    ) {
                        self.open();
                    }
                }
            }
            MouseEventKind::Drag(MouseButton::Left) => {
                // Promote pending to a real selection on first drag.
                if let Some(pending) = self.pending_selection.take() {
                    self.view.selection = Some((
                        pending.pane,
                        Selection::start(pending.column, pending.anchor),
                    ));
                }
                // Update existing selection.
                if let Some((pane_id, ref sel)) = self.view.selection
                    && let Some(ta) = self.screen_map.text_area_of(pane_id, sel.column)
                {
                    let pos = ta.to_pos(
                        mouse.column,
                        mouse.row,
                        &self.view.tab().pane(pane_id).viewport,
                    );
                    let view_lines = self
                        .view
                        .buffer(self.view.tab().pane(pane_id).buffer)
                        .view_lines();
                    let pos = Pos::new(pos.line.min(view_lines.saturating_sub(1)), pos.col);
                    self.view.selection.as_mut().unwrap().1.update(pos);
                }
            }
            MouseEventKind::Up(MouseButton::Left) => {
                // A click without drag — no selection created.
                self.pending_selection = None;
                // If selection exists but is empty (drag returned to anchor), clear.
                if self
                    .view
                    .selection
                    .as_ref()
                    .is_some_and(|(_, sel)| sel.is_empty())
                {
                    self.view.selection = None;
                }
            }
            _ => {}
        }
        Flow::Continue
    }
}
