//! Tests for the Gutter component.

use loom::testing::Harness;
use ratatui::style::{Color, Style};
use ui::components::gutter::{Gutter, GutterProps};

fn gutter(number: Option<u32>, width: u16) -> Harness {
    let style = Style::default().fg(Color::White).bg(Color::DarkGray);
    let blank = Style::default().bg(Color::Black);
    let mut h = Harness::new::<Gutter>(
        GutterProps {
            number,
            style,
            blank,
            width,
        },
        width,
        1,
    );
    h.draw();
    h
}

#[test]
fn a_line_number_is_right_aligned() {
    let mut h = gutter(Some(42), 5);
    let line = h.screen_line(0);
    // Width 5: "  42 " — spaces, digits, one trailing space.
    assert!(
        line.starts_with("  42"),
        "right-aligned with trailing space: {:?}",
        line
    );
}

#[test]
fn a_single_digit_sits_at_the_right_edge() {
    let mut h = gutter(Some(7), 4);
    let line = h.screen_line(0);
    assert!(line.contains("7"), "got {:?}", line);
}

#[test]
fn a_blank_gutter_has_no_digits() {
    let mut h = gutter(None, 4);
    let line = h.screen_line(0);
    assert!(
        !line.chars().any(|c| c.is_ascii_digit()),
        "blank has no digits: {:?}",
        line
    );
}

#[test]
fn the_blank_gutter_uses_the_blank_style() {
    let mut h = gutter(None, 4);
    let bg = h.style_at(0, 0).bg;
    assert_eq!(bg, Some(Color::Black), "blank background");
}

#[test]
fn the_number_gutter_uses_the_number_style() {
    let mut h = gutter(Some(1), 4);
    let bg = h.style_at(0, 0).bg;
    assert_eq!(bg, Some(Color::DarkGray), "number background");
}

#[test]
fn a_wide_number_fills_the_gutter() {
    let mut h = gutter(Some(99999), 5);
    let line = h.screen_line(0);
    assert!(line.contains("99999"), "the number fits: {:?}", line);
}
