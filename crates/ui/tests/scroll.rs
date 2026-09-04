//! Tests for the use_scroll hook.

use ui::hooks::use_scroll::top_with_line_visible;

const MARGIN: u32 = 3;

#[test]
fn a_visible_line_does_not_move_the_viewport() {
    assert_eq!(top_with_line_visible(5, 20, 10, MARGIN, 0), 0);
}

#[test]
fn a_line_below_the_viewport_is_kept_visible() {
    let top = top_with_line_visible(15, 20, 10, MARGIN, 0);
    assert!(top > 0, "the view moved to show row 15, got top {top}");
    assert!(15 >= top && 15 < top + 10, "row 15 is on screen");
}

#[test]
fn a_line_above_the_viewport_is_kept_visible() {
    let top = top_with_line_visible(0, 20, 10, MARGIN, 12);
    assert_eq!(top, 0, "the view came back to show the line");
}

#[test]
fn keeping_a_line_visible_uses_the_requested_margin() {
    let top = top_with_line_visible(7, 20, 10, MARGIN, 0);
    assert_eq!(top, 1, "three rows are kept after the line");
}

#[test]
fn keeping_a_line_visible_never_scrolls_past_the_end() {
    let top = top_with_line_visible(11, 12, 10, MARGIN, 0);
    assert_eq!(top, 2, "total 12, height 10 → max top is 2");
}

#[test]
fn keeping_a_line_visible_in_short_content_keeps_the_viewport_at_zero() {
    assert_eq!(top_with_line_visible(2, 3, 10, MARGIN, 0), 0);
    assert_eq!(top_with_line_visible(0, 1, 10, MARGIN, 0), 0);
}

#[test]
fn keeping_a_line_visible_with_zero_height_returns_zero() {
    assert_eq!(top_with_line_visible(5, 20, 0, MARGIN, 0), 0);
}
