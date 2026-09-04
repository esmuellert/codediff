//! Shared vertical viewport state.

use std::collections::HashMap;
use std::ops::Range;

use loom::{NodeHandle, Ref, Scope, SetState, use_measure, use_ref, use_state};

/// The values a component reads during render: where to look and
/// what to attach to its host element.
pub struct ScrollView {
    pub top: u32,
    pub width: u16,
    pub view_lines: Range<u32>,
    pub node_ref: Ref<Option<NodeHandle>>,
}

/// A `Copy` handle for closures. Every viewport movement lives here.
#[derive(Clone, Copy)]
pub struct ScrollHandle {
    total: u32,
    height: u32,
    set_top: SetState<u32>,
}

impl ScrollHandle {
    /// Move the viewport by a signed number of rows.
    pub fn scroll_by(self, rows: i32) {
        let last_top = self.total.saturating_sub(self.height);
        let down = rows.is_positive();
        let step = rows.unsigned_abs();
        (self.set_top)(&move |top| {
            let top = top.min(last_top);
            if down {
                top.saturating_add(step).min(last_top)
            } else {
                top.saturating_sub(step)
            }
        });
    }

    /// Move the viewport as little as possible to keep one row visible.
    pub fn keep_line_visible(self, line: u32, margin: u32) {
        let total = self.total;
        let height = self.height;
        (self.set_top)(&move |top| top_with_line_visible(line, total, height, margin, top));
    }
}

/// Creates vertical viewport state. When `key` changes, the old position is
/// saved and the new one is restored.
pub fn use_scroll(scope: &mut Scope, key: Option<&str>, total: u32) -> (ScrollView, ScrollHandle) {
    let (requested_top, set_top) = use_state(scope, || 0u32);
    let (node_ref, size) = use_measure(scope);
    let height = u32::from(size.height);

    let positions = use_ref(scope, HashMap::<String, u32>::new);
    let previous_key = use_ref(scope, || None::<String>);

    let mut requested_top = requested_top;
    if let Some(current_key) = key {
        let changed = previous_key.current().as_deref() != Some(current_key);
        if changed {
            if let Some(previous_key) = previous_key.current().as_ref() {
                positions
                    .current()
                    .insert(previous_key.clone(), requested_top);
            }
            requested_top = positions
                .current()
                .get(current_key)
                .copied()
                .unwrap_or_default();
            set_top(&move |_| requested_top);
            *previous_key.current() = Some(current_key.to_string());
        }
    }

    let top = requested_top.min(total.saturating_sub(height));
    let view = ScrollView {
        top,
        width: size.width,
        view_lines: top..top.saturating_add(height).min(total),
        node_ref,
    };
    let handle = ScrollHandle {
        total,
        height,
        set_top,
    };

    (view, handle)
}

/// Returns the first row needed to keep `line` visible with a margin.
pub fn top_with_line_visible(
    line: u32,
    total: u32,
    height: u32,
    margin: u32,
    previous_top: u32,
) -> u32 {
    if height == 0 {
        return 0;
    }
    let last_top = total.saturating_sub(height);
    let line = line.min(total.saturating_sub(1));
    let margin = margin.min(height.saturating_sub(1) / 2);

    let mut top = previous_top.min(last_top);
    if line < top + margin {
        top = line.saturating_sub(margin);
    }
    if line + margin >= top + height {
        top = (line + margin + 1).saturating_sub(height);
    }
    top.min(last_top)
}
