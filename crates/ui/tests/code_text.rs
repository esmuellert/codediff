//! Tests for the CodeText component.

use std::ops::Range;
use std::rc::Rc;

use loom::testing::Harness;
use ratatui::style::{Color, Style};
use ui::Theme;
use ui::components::code_text::{CodeText, CodeTextProps};
use ui::components::{Context, Ui};

fn code_text(
    text: &str,
    diff: Vec<Range<u32>>,
    width: u16,
    unchanged: Style,
    changed: Style,
) -> Harness {
    let mut h = Harness::new::<CodeText>(
        CodeTextProps {
            text: Rc::from(text),
            diff: Rc::from(diff.as_slice()),
            syntax: Rc::from([]),
            unchanged_style: unchanged,
            changed_style: changed,
            selection: None,
        },
        width, 1,
    ).provide::<Ui>(Context {
        theme: Rc::new(Theme::DARK),
        ..Context::default()
    });
    h.draw();
    h
}

#[test]
fn the_text_appears_on_screen() {
    let mut h = code_text("hello world", vec![], 20, Style::default(), Style::default());
    let row = h.screen_row(0);
    assert!(row.contains("hello world"), "got {:?}", row);
}

#[test]
fn unchanged_bytes_get_the_unchanged_background() {
    let unchanged = Style::default().bg(Color::Blue);
    let changed = Style::default().bg(Color::Red);
    let mut h = code_text("hello", vec![], 10, unchanged, changed);
    let bg = h.style_at(0, 0).bg;
    assert_eq!(bg, Some(Color::Blue), "unchanged background");
}

#[test]
fn changed_bytes_get_the_changed_background() {
    let unchanged = Style::default().bg(Color::Blue);
    let changed = Style::default().bg(Color::Red);
    // Bytes 0..5 are changed (the whole word "hello").
    let mut h = code_text("hello", vec![0..5], 10, unchanged, changed);
    let bg = h.style_at(0, 0).bg;
    assert_eq!(bg, Some(Color::Red), "changed background");
}

#[test]
fn a_partial_diff_colours_only_the_changed_range() {
    let unchanged = Style::default().bg(Color::Blue);
    let changed = Style::default().bg(Color::Red);
    // "hello world" — bytes 6..11 ("world") are changed.
    let mut h = code_text("hello world", vec![6..11], 20, unchanged, changed);
    let hello_bg = h.style_at(0, 0).bg;
    let world_bg = h.style_at(6, 0).bg;
    assert_eq!(hello_bg, Some(Color::Blue), "unchanged part");
    assert_eq!(world_bg, Some(Color::Red), "changed part");
}

#[test]
fn the_background_fills_past_the_end_of_the_text() {
    let unchanged = Style::default().bg(Color::Blue);
    let mut h = code_text("hi", vec![], 10, unchanged, Style::default());
    let past_end = h.style_at(5, 0).bg;
    assert_eq!(past_end, Some(Color::Blue), "the background extends past the text");
}
