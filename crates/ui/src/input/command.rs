//! What a resolved key sequence asks for, and who will do it.
//!
//! [`Action`] has one arm per **executor**. Four of them are the levels of the
//! view model, and the rule that decides which level a command belongs to is:
//!
//! > An action is executed by the **lowest level that contains everything it
//! > affects.**
//!
//! A motion affects one viewport, so the buffer does it. Resizing a border
//! affects *two* panes, so only the tab can. The executor hierarchy **is** the
//! containment hierarchy — there is nothing extra to remember, and each level
//! owns its own commands in its own file. See D27.
//!
//! Nothing here is bound to a key; each level binds its own. This file is only
//! the routing.

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
/// The first four arms are the view model, innermost first, and lookup
/// consults their bindings in that order. `Program` is not a level: it sits
/// below every one of them.
///
/// | arm | executed by | can fail | latency |
/// |---|---|---|---|
/// | `Buffer` | the focused pane's buffer | no | µs |
/// | `Pane` | the focused pane | no | µs |
/// | `Tab` | the active tab | no | µs |
/// | `View` | the view | no | µs |
/// | `Program` | whoever owns the terminal | no | µs |
///
/// Nothing here blocks and nothing leaves the crate. There used to be a sixth
/// level, `Task`, for the one action that needed a repository; it was returned
/// rather than run, because `ui` could not reach git. The pipeline answers on
/// a thread now, so asking costs a `send` and the level is gone. See D59.
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
