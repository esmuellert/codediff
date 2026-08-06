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
    /// Colour the code, or stop.
    ///
    /// Here rather than at the buffer level because it is the reader's
    /// preference for the whole session: turning it off in one buffer and
    /// finding it on in the next would read as a bug. What it is *for* is a
    /// direct comparison — syntax colour and diff colour share one small
    /// palette, and being able to remove one of them is how you find out
    /// whether the other still reads.
    ToggleSyntax,
    /// Open what the list has selected.
    ///
    /// Here rather than at the buffer level because it replaces the buffer in
    /// the *other* pane, which no buffer can do to itself. The row may turn
    /// out to be a directory, in which case the list folds it and the view is
    /// untouched — which of the two it is, is the buffer's answer and not the
    /// key's: one key does the obvious thing on every row, exactly as it does
    /// in the plugin.
    ///
    /// It used to be a `Task`, returned out of the crate for the composition
    /// root to perform, because performing it meant reaching git. It is
    /// performed here now: the pipeline answers on a thread of its own, so
    /// asking costs a `send` and nothing here waits. See D59.
    Open,
}

pub const BINDINGS: &[Binding] = &[
    view(&[key!(t)], ViewAction::ToggleLayout),
    view(&[key!(s)], ViewAction::ToggleSyntax),
];

const fn view(keys: &'static [KeyCombination], action: ViewAction) -> Binding {
    Binding {
        keys,
        action: Action::View(action),
    }
}
