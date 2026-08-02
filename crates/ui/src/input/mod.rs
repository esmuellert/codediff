//! Keys to commands.
//!
//! ---
//!
//! Admission criterion: does this decide *what was asked for*? Never what to
//! do about it. [`Resolver`] resolves and returns; the loop dispatches. If
//! this module also acted, it would need references to the view, the terminal
//! and the task runner all at once, which is the coupling the three-way
//! [`Action`] exists to avoid.
//!
//! One file per **executor**, each holding that executor's commands *and* the
//! keys bound to them — so a new command is one file, not two:
//!
//! ```text
//! buffer.rs    motions, and whatever a buffer kind adds   ← innermost
//! pane.rs      one pane, about its own view of a buffer
//! tab.rs       a tab, about its panes: focus, resize, zoom
//! view.rs      the whole view, about its tabs             ← outermost
//! program.rs   quit, suspend, redraw — below every level
//! task.rs      what leaves the crate
//! ```
//!
//! The first four are the view model, and lookup walks them in that order, so
//! an inner level shadows an outer one. The machinery is the rest:
//!
//! | file | question it answers |
//! |---|---|
//! | [`command`] | what was asked for, and who will do it |
//! | [`keymap`] | how a sequence is looked up |
//! | `resolver` | how a stream of keys becomes one command |

pub mod buffer;
pub mod command;
pub mod keymap;
pub mod pane;
pub mod program;
mod resolver;
pub mod tab;
pub mod task;
pub mod view;

use crokey::KeyCombination;
use crokey::crossterm::event::{Event, KeyEventKind};

pub use buffer::{BufferAction, DIVIDER_STEP, Motion, SCROLL_STEP};
pub use command::{Action, Command};
pub use keymap::{Binding, Context, Match};
pub use pane::PaneAction;
pub use program::ProgramAction;
pub use resolver::{Resolution, Resolver};
pub use tab::TabAction;
pub use task::TaskAction;
pub use view::ViewAction;

/// The key of a press, normalised, or `None` for anything else.
///
/// Windows reports releases as well as presses; without this every key would
/// act twice there and nowhere else. Repeats are kept, so holding `j` scrolls.
pub fn press(event: &Event) -> Option<KeyCombination> {
    match event {
        Event::Key(key) if matches!(key.kind, KeyEventKind::Press | KeyEventKind::Repeat) => {
            Some(KeyCombination::from(*key).normalized())
        }
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crokey::key;

    #[test]
    fn a_key_release_is_not_a_press() {
        use crokey::crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
        let mut event = KeyEvent::new(KeyCode::Char('j'), KeyModifiers::NONE);
        event.kind = KeyEventKind::Release;
        assert_eq!(press(&Event::Key(event)), None);

        event.kind = KeyEventKind::Repeat;
        assert_eq!(
            press(&Event::Key(event)),
            Some(key!(j)),
            "holding j scrolls"
        );
    }

    #[test]
    fn a_resize_is_not_a_key() {
        assert_eq!(press(&Event::Resize(80, 24)), None);
    }
}
