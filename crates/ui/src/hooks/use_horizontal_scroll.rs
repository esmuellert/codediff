//! Shared horizontal viewport state.

use std::collections::HashMap;

use loom::{Scope, SetState, use_ref, use_state};

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
        (self.set_requested_first_cell)(&|_| 0);
    }

    pub fn scroll_to_end(self) {
        let first_cell = self.maximum_first_cell;
        (self.set_requested_first_cell)(&move |_| first_cell);
    }
}

pub fn use_horizontal_scroll(
    scope: &mut Scope,
    file_key: Option<&str>,
    maximum_first_cell: u32,
) -> (HorizontalScrollView, HorizontalHandle) {
    let (requested_first_cell, set_requested_first_cell) = use_state(scope, || 0u32);
    let horizontal_positions = use_ref(scope, HashMap::<String, u32>::new);
    let previous_file_key = use_ref(scope, || None::<String>);

    let mut requested_first_cell = requested_first_cell;
    if let Some(file_key) = file_key {
        let changed = previous_file_key.current().as_deref() != Some(file_key);
        if changed {
            if let Some(previous_file_key) = previous_file_key.current().as_ref() {
                horizontal_positions
                    .current()
                    .insert(previous_file_key.clone(), requested_first_cell);
            }
            requested_first_cell = horizontal_positions
                .current()
                .get(file_key)
                .copied()
                .unwrap_or(0);
            set_requested_first_cell(&move |_| requested_first_cell);
            *previous_file_key.current() = Some(file_key.to_owned());
        }
    }

    let first_cell = requested_first_cell.min(maximum_first_cell);
    let view = HorizontalScrollView { first_cell };
    let handle = HorizontalHandle {
        maximum_first_cell,
        set_requested_first_cell,
    };
    (view, handle)
}
