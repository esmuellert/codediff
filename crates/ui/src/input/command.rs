//! The `Command` and `Action` types dispatched by the event loop.
//!
//! One arm per executor level. The rule: an action is executed by the lowest
//! level that contains everything it affects.

use std::num::NonZeroU32;

use crate::input::buffer::BufferAction;
use crate::input::pane::PaneAction;
use crate::input::program::ProgramAction;
use crate::input::tab::TabAction;
use crate::input::view::ViewAction;

/// One resolved key sequence: what to do, and how many times.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Command {
    /// The digits typed before the keys, as in `5j`.
    ///
    /// What it *means* is the command's business, exactly as in vim: `5j` is
    /// five downs, while `5G` is line five.
    pub count: Option<NonZeroU32>,
    pub action: Action,
}

impl Command {
    pub const fn new(action: Action) -> Self {
        Self {
            count: None,
            action,
        }
    }

    /// The count, or 1 — for the commands that simply repeat.
    pub fn repeat(&self) -> u32 {
        self.count.map_or(1, NonZeroU32::get)
    }
}

/// Who carries a command out.
///
/// The first four arms are the view model levels, innermost first.
/// `Program` sits below all of them. Nothing here blocks.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Action {
    /// Motions, and whatever this buffer's kind adds.
    Buffer(BufferAction),
    /// One pane, about its own view of a buffer.
    Pane(PaneAction),
    /// A tab, about its panes — anything affecting more than one of them.
    Tab(TabAction),
    /// The whole view, about its tabs.
    View(ViewAction),
    Program(ProgramAction),
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::input::buffer::Motion;

    #[test]
    fn a_command_without_a_count_repeats_once() {
        assert_eq!(
            Command::new(Action::Buffer(BufferAction::Motion(Motion::Down))).repeat(),
            1
        );
    }

    #[test]
    fn a_count_is_the_number_of_repeats() {
        let command = Command {
            count: NonZeroU32::new(5),
            action: Action::Buffer(BufferAction::Motion(Motion::Down)),
        };
        assert_eq!(command.repeat(), 5);
    }
}
