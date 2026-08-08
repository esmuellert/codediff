//! Handling keys and mouse events: translate them into actions on the view.

use crossterm::event::MouseEventKind;
use ratatui::layout::Rect;

use crate::input::{Action, Command, ProgramAction, Resolution, TabAction, ViewAction};
use crate::render::layout;
use crate::view::{BufferType, Layout, PaneId};

use super::{Flow, Session};

impl Session {
    /// Applies one key (for tests — `handle` wraps the crossterm event).
    pub fn press(&mut self, key: crokey::KeyCombination) -> Flow {
        self.notice = None;
        match self.resolver.key(key, self.view.keymap_type()) {
            Resolution::Run(command) => self.dispatch(command),
            Resolution::Pending | Resolution::Cancelled | Resolution::Unbound => Flow::Continue,
        }
    }

    /// Applies one terminal event — key or mouse.
    pub fn handle_event(&mut self, event: &crossterm::event::Event) -> Flow {
        use crossterm::event::Event;

        match event {
            Event::Key(_) => {
                let Some(key) = crate::input::press(event) else {
                    return Flow::Continue;
                };
                self.press(key)
            }
            Event::Mouse(mouse) => {
                match mouse.kind {
                    MouseEventKind::ScrollUp | MouseEventKind::ScrollDown => {
                        // Scroll the pane the mouse is hovering over, not the
                        // focused one — and move the view without moving the
                        // cursor, the way a browser does.
                        let delta: i32 = if mouse.kind == MouseEventKind::ScrollUp {
                            -3
                        } else {
                            3
                        };
                        let col = mouse.column;
                        let row = mouse.row;
                        if let Some((pane_id, _)) = self.hit_map.panes.iter().find(|(_, rect)| {
                            col >= rect.x
                                && col < rect.x + rect.width
                                && row >= rect.y
                                && row < rect.y + rect.height
                        }) {
                            let pane_id = *pane_id;
                            let pane = self.view.tab().pane(pane_id);
                            let view_lines = self.view.buffer(pane.buffer).view_lines();
                            self.view
                                .tab_mut()
                                .pane_mut(pane_id)
                                .viewport
                                .scroll(delta, view_lines);
                        }
                    }
                    MouseEventKind::Down(crossterm::event::MouseButton::Left) => {
                        let col = mouse.column;
                        let row = mouse.row;
                        if let Some((pane_id, area)) =
                            self.hit_map.panes.iter().find(|(_, rect)| {
                                col >= rect.x
                                    && col < rect.x + rect.width
                                    && row >= rect.y
                                    && row < rect.y + rect.height
                            })
                        {
                            let pane_id = *pane_id;
                            let area = *area;
                            self.view.tab_mut().set_focus(pane_id);
                            let line_in_pane = (row - area.y) as u32;
                            let (buffer, viewport) = self.view.focused_mut();
                            let target = viewport.top() + line_in_pane;
                            let clamped = target.min(buffer.view_lines().saturating_sub(1));
                            viewport.place(clamped, buffer.view_lines());
                            if matches!(buffer.buffer_type(), BufferType::Explorer(_)) {
                                self.open();
                            }
                        }
                    }
                    _ => {}
                }
                Flow::Continue
            }
            _ => Flow::Continue,
        }
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

    /// Routes a command to its executor.
    fn dispatch(&mut self, command: Command) -> Flow {
        match command.action {
            Action::Buffer(action) => {
                let count = command.repeat();
                let (buffer, viewport) = self.view.focused_mut();
                buffer.act(action, count, viewport);
                Flow::Continue
            }
            Action::Pane(action) => match action {},
            Action::Tab(TabAction::FocusNext | TabAction::FocusPrev) => {
                self.view.tab_mut().focus_next();
                Flow::Continue
            }
            Action::Tab(TabAction::WidenLeft | TabAction::NarrowLeft) => Flow::Continue,
            Action::View(ViewAction::ToggleLayout) => {
                self.view.toggle_layout();
                Flow::Continue
            }
            Action::View(ViewAction::ToggleSyntax) => {
                self.view.toggle_syntax();
                Flow::Continue
            }
            Action::View(ViewAction::Open) => {
                let (buffer, viewport) = self.view.focused_mut();
                let cursor = viewport.cursor();
                if buffer.select(cursor) {
                    let lines = buffer.view_lines();
                    viewport.place(cursor.min(lines.saturating_sub(1)), lines);
                } else {
                    self.open();
                }
                Flow::Continue
            }
            Action::Program(ProgramAction::Quit) => Flow::Quit,
            Action::Program(ProgramAction::Suspend) => Flow::Suspend,
        }
    }
}
