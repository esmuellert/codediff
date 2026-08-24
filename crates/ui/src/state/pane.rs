//! One buffer, and one position onto it.
//!
//! A pane does not know its own size — the tab computes rectangles. What a
//! pane holds is what belongs to *this view of this buffer*: position today,
//! window-local settings (wrap, line numbers) later.

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
