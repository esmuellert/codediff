//! What a tab can do, and the keys that ask for it.
//!
//! Focus, resize, zoom — everything affecting more than one pane. An action
//! is executed by the lowest level containing everything it affects.

use crokey::{KeyCombination, key};

use crate::input::command::Action;
use crate::input::keymap::Binding;

/// Something a tab does to its panes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TabAction {
    /// Move the focus to the next pane.
    FocusNext,
    /// Give the left-hand pane more columns, and its neighbour fewer.
    WidenLeft,
    NarrowLeft,
}

/// Columns [`TabAction::WidenLeft`] and [`TabAction::NarrowLeft`] move, per
/// repeat.
pub const RESIZE_STEP: i16 = 4;

const fn tab(keys: &'static [KeyCombination], action: TabAction) -> Binding {
    Binding {
        keys,
        action: Action::Tab(action),
    }
}

/// `<Tab>` is the only key live at every level.
///
/// Moving the border is **not** here. A tab-level binding is live in every
/// buffer, and a plain file has no border beside it, so `>` there would be a
/// key that silently does nothing. The two resize keys are bound by the list
/// instead — they still name [`TabAction`], because the tab is what executes
/// them, and a binding's list and its executor need not be the same level.
pub const BINDINGS: &[Binding] = &[tab(&[key!(tab)], TabAction::FocusNext)];

pub const fn resize(keys: &'static [KeyCombination], action: TabAction) -> Binding {
    tab(keys, action)
}
