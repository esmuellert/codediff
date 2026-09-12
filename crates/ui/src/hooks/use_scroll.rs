//! Shared vertical viewport state.

use std::ops::Range;

use loom::{Scope, SetState, use_effect, use_ref, use_state};

/// The values a component reads during render: where to look.
pub struct ScrollView {
    pub top: u32,
    pub view_lines: Range<u32>,
}

/// A `Copy` handle for closures. Every viewport movement lives here.
#[derive(Clone, Copy)]
pub struct ScrollHandle {
    line_count: u32,
    height: u32,
    set_top: SetState<u32>,
}

impl ScrollHandle {
    /// Move the viewport to an absolute row.
    pub fn scroll_to(self, top: u32) {
        let last_top = self.line_count.saturating_sub(self.height);
        (self.set_top)(&move |_| top.min(last_top));
    }

    /// Move the viewport by a signed number of lines.
    pub fn scroll_by(self, lines: i32) {
        let last_top = self.line_count.saturating_sub(self.height);
        let down = lines.is_positive();
        let step = lines.unsigned_abs();
        (self.set_top)(&move |top| {
            let top = top.min(last_top);
            if down {
                top.saturating_add(step).min(last_top)
            } else {
                top.saturating_sub(step)
            }
        });
    }

    /// Move the viewport as little as possible to keep one line visible.
    pub fn keep_line_visible(self, line: u32, margin: u32) {
        let line_count = self.line_count;
        let height = self.height;
        (self.set_top)(&move |top| top_with_line_visible(line, line_count, height, margin, top));
    }
}

/// Creates vertical viewport state, starting at `initial_top`.
///
/// The hook owns only the active viewport. A component that needs history
/// supplies the initial position and stores its own state.
pub fn use_scroll(
    scope: &mut Scope,
    line_count: u32,
    initial_top: u32,
    height: u16,
) -> (ScrollView, ScrollHandle) {
    let (requested_top, set_top) = use_state(scope, || initial_top);
    let height = u32::from(height);
    let signature = (line_count, height);
    let previous_signature = use_ref(scope, || signature);
    let dimensions_changed = *previous_signature.current() != signature;
    *previous_signature.current() = signature;
    use_effect(scope, signature, move || {
        set_top(&move |_| initial_top);
    });

    let requested_top = if dimensions_changed {
        initial_top
    } else {
        requested_top
    };
    let top = requested_top.min(line_count.saturating_sub(height));
    let view = ScrollView {
        top,
        view_lines: top..top.saturating_add(height).min(line_count),
    };
    let handle = ScrollHandle {
        line_count,
        height,
        set_top,
    };

    (view, handle)
}

/// Returns the first line needed to keep `line` visible with a margin.
pub fn top_with_line_visible(
    line: u32,
    line_count: u32,
    height: u32,
    margin: u32,
    previous_top: u32,
) -> u32 {
    if height == 0 {
        return 0;
    }
    let last_top = line_count.saturating_sub(height);
    let line = line.min(line_count.saturating_sub(1));
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
