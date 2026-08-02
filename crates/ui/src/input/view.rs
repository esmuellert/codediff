//! What the whole view can do, and the keys that ask for it.
//!
//! Tabs: opening, closing, cycling. The outermost level of the view model, and
//! so the last consulted before [`program`](super::program) — a binding here is
//! shadowed by any level below that claims the same keys.
//!
//! Uninhabited until there is more than one tab.

use crate::input::keymap::Binding;

/// Something the view does to its tabs.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ViewAction {}

pub const BINDINGS: &[Binding] = &[];
