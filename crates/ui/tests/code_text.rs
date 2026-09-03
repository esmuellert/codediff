//! Tests for the CodeText component.

use std::ops::Range;
use std::rc::Rc;

use loom::testing::Harness;
use ratatui::style::{Color, Style};
use ui::Theme;
use ui::components::code_text::{CodeText, CodeTextProps};
use ui::components::{Context, Ui};

fn one_range(range: Range<u32>) -> Vec<Range<u32>> {
    std::iter::once(range).collect()
}

fn code_text(
    text: &str,
    diff: Vec<Range<u32>>,
    width: u16,
    unchanged: Style,
    changed: Style,
) -> Harness {
    decorated_code_text(text, diff, None, Vec::new(), width, unchanged, changed)
}

fn decorated_code_text(
    text: &str,
    diff: Vec<Range<u32>>,
    fill_from: Option<u32>,
    empty_markers: Vec<u32>,
    width: u16,
    unchanged: Style,
    changed: Style,
) -> Harness {
    code_text_harness(CodeTextInput {
        text,
        first_cell: 0,
        diff,
        fill_from,
        empty_markers,
        syntax: Vec::new(),
        width,
        unchanged,
        changed,
    })
}

struct CodeTextInput<'a> {
    text: &'a str,
    first_cell: u32,
    diff: Vec<Range<u32>>,
    fill_from: Option<u32>,
    empty_markers: Vec<u32>,
    syntax: Vec<syntax::Span>,
    width: u16,
    unchanged: Style,
    changed: Style,
}

fn code_text_harness(input: CodeTextInput<'_>) -> Harness {
    let mut h = Harness::new::<CodeText>(
        CodeTextProps {
            text: Rc::from(input.text),
            first_cell: input.first_cell,
            diff: Rc::from(input.diff.as_slice()),
            fill_from: input.fill_from,
            empty_markers: Rc::from(input.empty_markers.as_slice()),
            syntax: Rc::from(input.syntax.as_slice()),
            unchanged_style: input.unchanged,
            changed_style: input.changed,
            selection: None,
        },
        input.width,
        1,
    )
    .provide::<Ui>(Context {
        theme: Rc::new(Theme::DARK),
        ..Context::default()
    });
    h.draw();
    h
}

fn scrolled_text(text: &str, first_cell: u32, width: u16) -> Harness {
    code_text_harness(CodeTextInput {
        text,
        first_cell,
        diff: Vec::new(),
        fill_from: None,
        empty_markers: Vec::new(),
        syntax: Vec::new(),
        width,
        unchanged: Style::default(),
        changed: Style::default(),
    })
}

#[test]
fn the_text_appears_on_screen() {
    let mut h = code_text(
        "hello world",
        vec![],
        20,
        Style::default(),
        Style::default(),
    );
    let row = h.screen_row(0);
    assert!(row.contains("hello world"), "got {:?}", row);
}

#[test]
fn horizontal_start_selects_the_painted_cell_window() {
    let mut harness = scrolled_text("abcdef", 2, 4);

    assert_eq!(harness.screen_row(0), "cdef");
}

#[test]
fn horizontal_start_does_not_split_a_wide_character() {
    let mut harness = scrolled_text("a日bc", 2, 3);

    assert_eq!(harness.screen_row(0), " bc");
}

#[test]
fn tabs_keep_their_cell_positions_after_horizontal_scrolling() {
    let mut harness = scrolled_text("a\tbc", 2, 4);

    assert_eq!(harness.screen_row(0), "  bc");
}

#[test]
fn diff_styles_follow_their_characters_after_horizontal_scrolling() {
    let unchanged = Style::default().bg(Color::Blue);
    let changed = Style::default().bg(Color::Red);
    let mut harness = code_text_harness(CodeTextInput {
        text: "abcdef",
        first_cell: 2,
        diff: one_range(3..4),
        fill_from: None,
        empty_markers: Vec::new(),
        syntax: Vec::new(),
        width: 3,
        unchanged,
        changed,
    });

    assert_eq!(harness.style_at(0, 0).bg, Some(Color::Blue));
    assert_eq!(harness.style_at(1, 0).bg, Some(Color::Red));
    assert_eq!(harness.style_at(2, 0).bg, Some(Color::Blue));
}

#[test]
fn syntax_styles_follow_their_characters_after_horizontal_scrolling() {
    let pen = syntax::Pen(0);
    let mut harness = code_text_harness(CodeTextInput {
        text: "abcdef",
        first_cell: 2,
        diff: Vec::new(),
        fill_from: None,
        empty_markers: Vec::new(),
        syntax: vec![syntax::Span::new(3..4, syntax::Style::pen(pen))],
        width: 3,
        unchanged: Style::default(),
        changed: Style::default(),
    });

    let plain = harness.style_at(0, 0).fg;
    assert_eq!(harness.style_at(1, 0).fg, Theme::DARK.code.pen(Some(pen)));
    assert_eq!(harness.style_at(2, 0).fg, plain);
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
    let mut h = code_text("hello", one_range(0..5), 10, unchanged, changed);
    let bg = h.style_at(0, 0).bg;
    assert_eq!(bg, Some(Color::Red), "changed background");
}

#[test]
fn a_partial_diff_colours_only_the_changed_range() {
    let unchanged = Style::default().bg(Color::Blue);
    let changed = Style::default().bg(Color::Red);
    // "hello world" — bytes 6..11 ("world") are changed.
    let mut h = code_text("hello world", one_range(6..11), 20, unchanged, changed);
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
    assert_eq!(
        past_end,
        Some(Color::Blue),
        "the background extends past the text"
    );
}

#[test]
fn a_range_crossing_the_line_break_fills_to_the_row_edge() {
    let unchanged = Style::default().bg(Color::Blue);
    let changed = Style::default().bg(Color::Red);
    let mut h = decorated_code_text(
        "hi",
        one_range(0..2),
        Some(0),
        Vec::new(),
        10,
        unchanged,
        changed,
    );

    assert_eq!(h.style_at(9, 0).bg, Some(Color::Red));
}

#[test]
fn a_whole_line_range_starts_before_a_zero_width_control_character() {
    let unchanged = Style::default().bg(Color::Blue);
    let changed = Style::default().bg(Color::Red);
    let mut h = decorated_code_text(
        "\r",
        one_range(1..1),
        Some(0),
        Vec::new(),
        10,
        unchanged,
        changed,
    );

    assert_eq!(h.style_at(0, 0).bg, Some(Color::Red));
}

#[test]
fn an_empty_range_draws_a_marker_without_claiming_a_character() {
    let unchanged = Style::default().bg(Color::Blue);
    let changed = Style::default().bg(Color::Red);
    let mut h = decorated_code_text("abc", Vec::new(), None, vec![1], 10, unchanged, changed);

    let marker = h.style_at(1, 0);
    assert_eq!(marker.bg, Some(Color::Blue));
    assert_eq!(marker.underline_color, Some(Color::Red));
}
