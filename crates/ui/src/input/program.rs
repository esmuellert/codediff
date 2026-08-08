//! Program-level actions (quit, suspend, redraw) and their keybindings.
//!
//! Not a level of the view model — it is below all of them, and consulted
//! last, because these are the keys that must work whatever is on screen.
//! Executed by whoever owns the terminal rather than by anything in `ui`.

use crokey::{KeyCombination, key};

use crate::input::command::Action;
use crate::input::keymap::Binding;

/// Executed by whoever owns the terminal. Cannot fail.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProgramAction {
    Quit,
    /// Hand the terminal back until the reader brings us forward.
    Suspend,
}

const fn program(keys: &'static [KeyCombination], action: ProgramAction) -> Binding {
    Binding {
        keys,
        action: Action::Program(action),
    }
}

pub const BINDINGS: &[Binding] = &[
    program(&[key!(q)], ProgramAction::Quit),
    // Bound to quit, but the resolver takes it first while a sequence or a
    // count is in flight — otherwise pressing `g` and changing your mind
    // would exit the program.
    program(&[key!(esc)], ProgramAction::Quit),
    program(&[key!(ctrl - c)], ProgramAction::Quit),
    program(&[key!(ctrl - z)], ProgramAction::Suspend),
];
