//! What a pane can do, and the keys that ask for it.
//!
//! Everything that belongs to *this view of this buffer* and to nothing else —
//! what Neovim calls window-local: `wrap`, line numbers, and the rest. The
//! distinction exists because one buffer can be shown in more than one pane, and
//! they must be able to disagree about how without disagreeing about what.
//!
//! There is no resize here. A pane shows one buffer, so it has no border
//! inside it; the border *between* panes belongs to the [tab](super::tab),
//! which is the lowest level containing both sides of it.
//!
//! Uninhabited.

use crate::input::keymap::Binding;

/// Something one pane does to itself.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PaneAction {}

pub const BINDINGS: &[Binding] = &[];
