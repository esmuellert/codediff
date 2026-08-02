//! What a tab can do, and the keys that ask for it.
//!
//! Focus, resize, zoom, show and hide — everything affecting *more than one
//! pane*. Resizing lives here rather than on a pane because growing one pane
//! must shrink its neighbour, and the tab is the lowest level containing both
//! sides of a border. It was the first command that could not sit where it
//! seemed to belong, and the rule it produced decides all the others: an
//! action is executed by the lowest level containing everything it affects.
//!
//! Uninhabited until the explorer brings a second pane.

use crate::input::keymap::Binding;

/// Something a tab does to its panes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TabAction {}

pub const BINDINGS: &[Binding] = &[];
