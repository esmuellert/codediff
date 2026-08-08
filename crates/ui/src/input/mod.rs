//! Keys to commands.
//!
//! One file per executor, each holding commands and keybindings:
//!
//! ```text
//! buffer.rs    motions, and whatever a buffer kind adds   ← innermost
//! pane.rs      one pane, about its own view of a buffer
//! tab.rs       a tab, about its panes: focus, resize
//! view.rs      the whole view, about its tabs             ← outermost
//! program.rs   quit, suspend, redraw — below every level
//! ```
//!
//! Lookup walks innermost-first, so inner levels shadow outer ones.

pub mod buffer;
pub mod command;
pub mod keymap;
pub mod pane;
pub mod program;
mod resolver;
pub mod tab;
pub mod view;

use crokey::KeyCombination;
use crokey::crossterm::event::{Event, KeyEventKind};

pub use buffer::{BufferAction, DIVIDER_STEP, Motion, SCROLL_STEP};
pub use command::{Action, Command};
pub use keymap::{Binding, KeymapType, Match};
pub use pane::PaneAction;
pub use program::ProgramAction;
pub use resolver::{Resolution, Resolver};
pub use tab::TabAction;
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
