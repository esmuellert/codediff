//! Shared horizontal viewport state.

use loom::{Scope, SetState, use_state};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct HorizontalScrollView {
    pub first_cell: u32,
}

#[derive(Clone, Copy)]
pub struct HorizontalHandle {
    maximum_first_cell: u32,
    set_requested_first_cell: SetState<u32>,
}

impl HorizontalHandle {
    /// Move the viewport to an absolute cell.
    pub fn scroll_to(self, first_cell: u32) {
        let maximum_first_cell = self.maximum_first_cell;
        (self.set_requested_first_cell)(&move |_| first_cell.min(maximum_first_cell));
    }

    /// Move the viewport by a signed number of cells.
    pub fn scroll_by(self, cells: i32) {
        let maximum_first_cell = self.maximum_first_cell;
        let right = cells.is_positive();
        let step = cells.unsigned_abs();
        (self.set_requested_first_cell)(&move |first_cell| {
            let first_cell = first_cell.min(maximum_first_cell);
            if right {
                first_cell.saturating_add(step).min(maximum_first_cell)
            } else {
                first_cell.saturating_sub(step)
            }
        });
    }

    pub fn scroll_to_start(self) {
        self.scroll_to(0);
    }

    pub fn scroll_to_end(self) {
        self.scroll_to(self.maximum_first_cell);
    }
}

pub fn use_horizontal_scroll(
    scope: &mut Scope,
    maximum_first_cell: u32,
    initial_first_cell: u32,
) -> (HorizontalScrollView, HorizontalHandle) {
    let (requested_first_cell, set_requested_first_cell) = use_state(scope, || initial_first_cell);

    let first_cell = requested_first_cell.min(maximum_first_cell);
    let view = HorizontalScrollView { first_cell };
    let handle = HorizontalHandle {
        maximum_first_cell,
        set_requested_first_cell,
    };
    (view, handle)
}
