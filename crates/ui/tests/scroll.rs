//! Tests for the use_scroll hook.

use ui::hooks::use_scroll::scroll_top;

// ---- scroll_top pure function ----

#[test]
fn scroll_top_stays_when_cursor_is_visible() {
    assert_eq!(scroll_top(5, 20, 10, 0), 0);
}

#[test]
fn scroll_top_follows_cursor_down() {
    let top = scroll_top(15, 20, 10, 0);
    assert!(top > 0, "the view moved to show row 15, got top {top}");
    assert!(15 >= top && 15 < top + 10, "row 15 is on screen");
}

#[test]
fn scroll_top_follows_cursor_back_up() {
    let top = scroll_top(0, 20, 10, 12);
    assert_eq!(top, 0, "the view came back with the cursor");
}

#[test]
fn scroll_top_keeps_a_margin() {
    let top = scroll_top(7, 20, 10, 0);
    assert_eq!(top, 1, "three rows kept past the cursor");
}

#[test]
fn scroll_top_never_goes_past_the_end() {
    let top = scroll_top(11, 12, 10, 0);
    assert_eq!(top, 2, "total 12, height 10 → max top is 2");
}

#[test]
fn scroll_top_with_short_content() {
    assert_eq!(scroll_top(2, 3, 10, 0), 0, "content fits, top stays 0");
    assert_eq!(scroll_top(0, 1, 10, 0), 0, "one item, top stays 0");
}

#[test]
fn scroll_top_with_zero_height() {
    assert_eq!(scroll_top(5, 20, 0, 0), 0, "zero height returns 0");
}
