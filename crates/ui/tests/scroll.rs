//! The UI uses Loom's generic two-dimensional scroll contract.

use loom::{ScrollMetrics, ScrollOffset};

#[test]
fn a_scroll_offset_clamps_to_the_measured_viewport() {
    let metrics = ScrollMetrics {
        content_width: 40,
        content_height: 20,
        viewport_width: 10,
        viewport_height: 5,
    };
    assert_eq!(
        ScrollOffset { x: 99, y: 99 }.clamp(metrics),
        ScrollOffset { x: 30, y: 15 }
    );
}
