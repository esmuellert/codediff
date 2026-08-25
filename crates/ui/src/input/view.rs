//! View-level actions and their keybindings.
//!
//! Tabs and buffers: the level that owns them both, and so the level where
//! anything replacing a buffer has to live. The outermost level of the view
//! model, and so the last consulted before [`program`](super::program) — a
//! binding here is shadowed by any level below that claims the same keys.

use crokey::{KeyCombination, key};

use crate::input::command::Action;
use crate::input::keymap::Binding;

/// Something the view does to its tabs or its buffers.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ViewAction {
    /// Read the focused diff the other way round: side by side, or inline.
    ToggleLayout,
    /// Open what the list has selected.
    Open,
}

pub const BINDINGS: &[Binding] = &[view(&[key!(t)], ViewAction::ToggleLayout)];

const fn view(keys: &'static [KeyCombination], action: ViewAction) -> Binding {
    Binding {
        keys,
        action: Action::View(action),
    }
}
