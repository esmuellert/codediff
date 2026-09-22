//! Generic two-dimensional scrolling state shared by the Scroll host and its
//! input handle.

use std::cell::Cell;
use std::rc::Rc;

use crate::hook::{Ref, SetState, use_ref, use_state};
use crate::scope::Scope;

/// A position in scrollable content, measured in terminal cells.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct ScrollOffset {
    pub x: u32,
    pub y: u32,
}

impl ScrollOffset {
    pub const ZERO: Self = Self { x: 0, y: 0 };

    pub fn clamp(self, metrics: ScrollMetrics) -> Self {
        Self {
            x: self.x.min(metrics.max_x()),
            y: self.y.min(metrics.max_y()),
        }
    }
}

/// The content and viewport sizes last discovered by layout.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct ScrollMetrics {
    pub content_width: u32,
    pub content_height: u32,
    pub viewport_width: u32,
    pub viewport_height: u32,
}

impl ScrollMetrics {
    pub fn max_x(self) -> u32 {
        self.content_width.saturating_sub(self.viewport_width)
    }

    pub fn max_y(self) -> u32 {
        self.content_height.saturating_sub(self.viewport_height)
    }
}

/// The render-time view passed to a [`Scroll`] host.
#[derive(Clone)]
pub struct ScrollView {
    /// The requested position, clamped to the metrics from the previous pass.
    pub offset: ScrollOffset,
    pub metrics: ScrollMetrics,
    pub(crate) requested: ScrollOffset,
    pub(crate) state: Rc<Cell<ScrollMetrics>>,
}

impl ScrollView {
    pub fn requested_offset(&self) -> ScrollOffset {
        self.requested
    }

    /// Returns a view that moves only along the selected axes.
    pub fn axes(mut self, horizontal: bool, vertical: bool) -> Self {
        if !horizontal {
            self.offset.x = 0;
            self.requested.x = 0;
        }
        if !vertical {
            self.offset.y = 0;
            self.requested.y = 0;
        }
        self
    }

    /// The current request clamped to the metrics written by layout.
    pub fn clamped_offset(&self) -> ScrollOffset {
        self.requested.clamp(self.state.get())
    }
}

impl Default for ScrollView {
    fn default() -> Self {
        Self {
            offset: ScrollOffset::ZERO,
            metrics: ScrollMetrics::default(),
            requested: ScrollOffset::ZERO,
            state: Rc::new(Cell::new(ScrollMetrics::default())),
        }
    }
}

/// A stable handle for moving one scroll view.
#[derive(Clone)]
pub struct ScrollHandle {
    set_offset: SetState<ScrollOffset>,
    state: Rc<Cell<ScrollMetrics>>,
}

impl ScrollHandle {
    /// Requests an absolute content position. Layout clamps it when the
    /// content or viewport is smaller than the request.
    pub fn scroll_to(&self, offset: ScrollOffset) {
        (self.set_offset)(&move |_| offset);
    }

    pub fn scroll_to_start(&self) {
        self.scroll_to(ScrollOffset::ZERO);
    }

    pub fn scroll_to_end(&self) {
        let metrics = self.state.get();
        self.scroll_to(ScrollOffset {
            x: metrics.max_x(),
            y: metrics.max_y(),
        });
    }

    pub fn scroll_x_to_start(&self) {
        let metrics = self.state.get();
        (self.set_offset)(&move |offset| ScrollOffset {
            x: 0,
            y: offset.y.min(metrics.max_y()),
        });
    }

    pub fn scroll_x_to_end(&self) {
        let metrics = self.state.get();
        (self.set_offset)(&move |offset| ScrollOffset {
            x: metrics.max_x(),
            y: offset.y.min(metrics.max_y()),
        });
    }

    /// Moves by signed cell deltas in content coordinates.
    pub fn scroll_by(&self, dx: i32, dy: i32) {
        let metrics = self.state.get();
        (self.set_offset)(&move |offset| {
            let offset = offset.clamp(metrics);
            ScrollOffset {
                x: move_by(offset.x, dx, metrics.max_x()),
                y: move_by(offset.y, dy, metrics.max_y()),
            }
        });
    }

    pub fn scroll_x_by(&self, cells: i32) {
        self.scroll_by(cells, 0);
    }

    pub fn scroll_y_by(&self, lines: i32) {
        self.scroll_by(0, lines);
    }

    /// Moves the vertical viewport just enough to show `line` with a margin.
    pub fn keep_y_visible(&self, line: u32, margin: u32) {
        let metrics = self.state.get();
        let height = metrics.viewport_height;
        if height == 0 || metrics.content_height == 0 {
            return;
        }
        let last_top = metrics.max_y();
        let line = line.min(metrics.content_height.saturating_sub(1));
        let margin = margin.min(height.saturating_sub(1) / 2);
        (self.set_offset)(&move |offset| {
            let mut top = offset.y.min(last_top);
            if line < top.saturating_add(margin) {
                top = line.saturating_sub(margin);
            }
            if line.saturating_add(margin) >= top.saturating_add(height) {
                top = line
                    .saturating_add(margin)
                    .saturating_add(1)
                    .saturating_sub(height);
            }
            ScrollOffset {
                x: offset.x,
                y: top.min(last_top),
            }
        });
    }

    pub fn metrics(&self) -> ScrollMetrics {
        self.state.get()
    }
}

/// Creates two-dimensional scroll state. The Scroll host writes its measured
/// metrics after layout; requests are intentionally retained until then so a
/// restored position can be applied after content arrives.
pub fn use_scroll(
    scope: &mut Scope,
    initial: impl FnOnce() -> ScrollOffset,
) -> (ScrollView, ScrollHandle) {
    let (requested, set_offset) = use_state(scope, initial);
    let metrics_ref: Ref<Rc<Cell<ScrollMetrics>>> =
        use_ref(scope, || Rc::new(Cell::new(ScrollMetrics::default())));
    let state = {
        let state = metrics_ref.current();
        Rc::clone(&state)
    };
    let metrics = state.get();
    let view = ScrollView {
        offset: requested.clamp(metrics),
        metrics,
        requested,
        state: Rc::clone(&state),
    };
    let handle = ScrollHandle { set_offset, state };
    (view, handle)
}

fn move_by(value: u32, delta: i32, maximum: u32) -> u32 {
    if delta.is_negative() {
        value.saturating_sub(delta.unsigned_abs())
    } else {
        value.saturating_add(delta.unsigned_abs()).min(maximum)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn offset_clamps_to_content_extent() {
        let metrics = ScrollMetrics {
            content_width: 30,
            content_height: 20,
            viewport_width: 10,
            viewport_height: 5,
        };
        assert_eq!(
            ScrollOffset { x: 99, y: 99 }.clamp(metrics),
            ScrollOffset { x: 20, y: 15 }
        );
    }

    #[test]
    fn moving_left_or_up_does_not_underflow() {
        assert_eq!(move_by(2, -9, 20), 0);
        assert_eq!(move_by(2, 4, 3), 3);
    }
}
