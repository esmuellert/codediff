use loom::{ScrollMetrics, ScrollOffset};

#[test]
fn offset_clamps_each_axis_to_its_content_extent() {
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
fn an_empty_or_smaller_content_has_no_scroll_range() {
    let metrics = ScrollMetrics {
        content_width: 4,
        content_height: 2,
        viewport_width: 10,
        viewport_height: 5,
    };
    assert_eq!(metrics.max_x(), 0);
    assert_eq!(metrics.max_y(), 0);
    assert_eq!(
        ScrollOffset { x: 9, y: 9 }.clamp(metrics),
        ScrollOffset::ZERO
    );
}
