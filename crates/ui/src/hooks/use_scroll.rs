//! Shared scroll and cursor state for scrollable panes.

use std::collections::HashMap;
use std::ops::Range;

use loom::{NodeHandle, Ref, Scope, SetState, use_measure, use_ref, use_state};

/// Rows kept between the cursor and the edge while scrolling.
const SCROLLOFF: u32 = 3;

/// The values a component reads during render: where to look and
/// what to attach to its host element.
pub struct ScrollView {
    pub cursor: u32,
    pub top: u32,
    pub width: u16,
    pub view_lines: Range<u32>,
    pub node_ref: Ref<Option<NodeHandle>>,
}

#[derive(Clone, Copy, Default, PartialEq, Eq)]
struct ScrollPosition {
    cursor: u32,
    top: u32,
}

/// A `Copy` handle for closures. Every movement method lives here.
#[derive(Clone, Copy)]
pub struct ScrollHandle {
    top: u32,
    height: u32,
    set_position: SetState<ScrollPosition>,
}

impl ScrollHandle {
    /// Move the cursor down by one. The view follows.
    pub fn down(self, total: u32) {
        let height = self.height;
        (self.set_position)(&move |position| {
            let cursor = position
                .cursor
                .saturating_add(1)
                .min(total.saturating_sub(1));
            ScrollPosition {
                cursor,
                top: scroll_top(cursor, total, height, position.top),
            }
        });
    }

    /// Move the cursor up by one. The view follows.
    pub fn up(self, total: u32) {
        let height = self.height;
        (self.set_position)(&move |position| {
            let cursor = position.cursor.saturating_sub(1);
            ScrollPosition {
                cursor,
                top: scroll_top(cursor, total, height, position.top),
            }
        });
    }

    /// Scroll the view without moving the cursor.
    pub fn wheel(self, delta: i32, total: u32) {
        let step = (delta.abs() * 3) as u32;
        let max_top = total.saturating_sub(self.height);
        (self.set_position)(&move |position| {
            let top = if delta > 0 {
                position.top.saturating_add(step).min(max_top)
            } else {
                position.top.saturating_sub(step)
            };
            ScrollPosition { top, ..position }
        });
    }

    /// Place the cursor at a screen row (view offset + local y).
    /// Returns the absolute line index.
    pub fn click(self, local_y: u32, total: u32) -> u32 {
        let line = (self.top + local_y).min(total.saturating_sub(1));
        (self.set_position)(&move |position| ScrollPosition {
            cursor: line,
            ..position
        });
        line
    }

    /// Place the cursor at an absolute position.
    pub fn set(self, cursor: u32) {
        (self.set_position)(&move |position| ScrollPosition { cursor, ..position });
    }
}

/// Creates the scroll state for one pane. When `key` changes, the old
/// position is saved and the new one is restored.
pub fn use_scroll(scope: &mut Scope, key: Option<&str>) -> (ScrollView, ScrollHandle) {
    let (position, set_position) = use_state(scope, ScrollPosition::default);
    let (node_ref, size) = use_measure(scope);
    let height = u32::from(size.height);

    let positions = use_ref(scope, HashMap::<String, ScrollPosition>::new);
    let prev_key = use_ref(scope, || None::<String>);

    if let Some(current_key) = key {
        let changed = prev_key.current().as_deref() != Some(current_key);
        if changed {
            // Save the old position.
            if let Some(old_key) = prev_key.current().as_ref() {
                positions.current().insert(old_key.clone(), position);
            }
            // Restore or start at zero.
            let restored = positions
                .current()
                .get(current_key)
                .copied()
                .unwrap_or_default();
            set_position(&move |_| restored);
            *prev_key.current() = Some(current_key.to_string());
        }
    }

    let view = ScrollView {
        cursor: position.cursor,
        top: position.top,
        width: size.width,
        view_lines: position.top..position.top + height,
        node_ref,
    };

    let handle = ScrollHandle {
        top: position.top,
        height,
        set_position,
    };

    (view, handle)
}

/// Computes the scroll top given a cursor, total rows, viewport height,
/// and the previous top. Keeps SCROLLOFF rows between the cursor and
/// the edges.
pub fn scroll_top(cursor: u32, total: u32, height: u32, prev_top: u32) -> u32 {
    if height == 0 {
        return 0;
    }
    let last_top = total.saturating_sub(height);
    let margin = SCROLLOFF.min(height.saturating_sub(1) / 2);

    let mut top = prev_top;
    if cursor < top + margin {
        top = cursor.saturating_sub(margin);
    }
    if cursor + margin >= top + height {
        top = (cursor + margin + 1).saturating_sub(height);
    }
    top.min(last_top)
}
