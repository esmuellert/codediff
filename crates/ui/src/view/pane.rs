//! One buffer, and one position onto it.
//!
//! A pane does **not** know its own size: the tab computes every rectangle,
//! because growing one pane must shrink its neighbour. What a pane holds is
//! what belongs to *this view of this buffer* and to nothing else — which
//! today is the position, and which is where window-local settings go when
//! they arrive.
//!
//! That distinction is Neovim's, and it exists for the same reason: `wrap`, `number`
//! and `cursorline` are window-local while `filetype` and `tabstop` are
//! buffer-local, because one buffer can be shown in several windows and they
//! must be able to disagree about how, but not about what.

use super::{BufferId, Viewport};

/// One buffer, and where the reader is looking at it.
#[derive(Debug)]
pub struct Pane {
    pub buffer: BufferId,
    pub viewport: Viewport,
}

impl Pane {
    pub fn new(buffer: BufferId) -> Self {
        Self {
            buffer,
            viewport: Viewport::new(),
        }
    }
}
