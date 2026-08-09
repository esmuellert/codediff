//! Key event handling: translate keys into actions on the view.

use crate::input::{Action, Command, ProgramAction, Resolution, TabAction, ViewAction};

use super::{Flow, Session};

impl Session {
    /// Applies one key (for tests — `handle_event` wraps the crossterm event).
    pub fn press(&mut self, key: crokey::KeyCombination) -> Flow {
        self.notice = None;
        match self.resolver.key(key, self.view.keymap_type()) {
            Resolution::Run(command) => self.dispatch(command),
            Resolution::Pending | Resolution::Cancelled | Resolution::Unbound => Flow::Continue,
        }
    }

    /// Routes a command to its executor.
    fn dispatch(&mut self, command: Command) -> Flow {
        match command.action {
            Action::Buffer(action) => {
                let count = command.repeat();
                let (buffer, viewport) = self.view.focused_mut();
                buffer.apply(action, count, viewport);
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
                if buffer.activate(cursor) {
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
