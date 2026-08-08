//! Pane-local settings (window-local in Neovim terms).
//!
//! Uninhabited — no pane-local settings exist yet.

use crate::input::keymap::Binding;

/// Something one pane does to itself.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PaneAction {}

pub const BINDINGS: &[Binding] = &[];
