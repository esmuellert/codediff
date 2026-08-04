//! What the whole view can do, and the keys that ask for it.
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
    ///
    /// Here rather than at the buffer level because it changes what the
    /// buffer *is* — its view-line count and its whole layout — and a buffer
    /// cannot be the thing that decides to replace itself. The view owns the
    /// buffers, so the view is the lowest level that contains the change.
    ToggleLayout,
}

pub const BINDINGS: &[Binding] = &[view(&[key!(t)], ViewAction::ToggleLayout)];

const fn view(keys: &'static [KeyCombination], action: ViewAction) -> Binding {
    Binding {
        keys,
        action: Action::View(action),
    }
}
