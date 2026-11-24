//! Tests for SideBySide.

use std::collections::HashMap;
use std::rc::Rc;
use std::sync::{Arc, Mutex};

use loom::testing::Harness;
use loom::{Node, Scope, component, rsx, use_ref};
use syntax::Span;
use ui::Theme;
use ui::components::diff_viewer::ScrollPosition;
use ui::components::diff_viewer_container::{DiffViewerContainer, DiffViewerContainerProps};
use ui::components::{Context, Ui};

use crate::fake_services::highlighted;

#[component]
fn TestSideBySide(
    scope: &mut Scope,
    content: Arc<diff::DiffContent>,
    colours: Arc<Mutex<[Vec<Vec<Span>>; 2]>>,
) -> Node {
    let scroll_positions = use_ref(scope, HashMap::<String, ScrollPosition>::new);
    let content_id = Arc::as_ptr(content) as usize;
    let key = content.file().path().as_str().to_owned();
    let active_scroll_position = use_ref(scope, ScrollPosition::default);
    let active_key = use_ref(scope, || None::<String>);
    let previous_key = active_key.current().clone();
    if previous_key.as_deref() != Some(key.as_str()) {
        if let Some(previous_key) = previous_key {
            let state = active_scroll_position.current().clone();
            scroll_positions.current().insert(previous_key, state);
        }
        *active_scroll_position.current() = scroll_positions
            .current()
            .get(&key)
            .cloned()
            .unwrap_or_default();
        *active_key.current() = Some(key);
    }
    rsx! {
        DiffViewerContainer {
            key: content_id,
            content: Some(Arc::clone(content)),
            colours: colours.clone(),
            viewer_mode: file_types::ViewerMode::SideBySide,
            scroll_position: active_scroll_position,
            wrap: true,
            compact: false,
            auto_focus: false,
        }
    }
}

#[component]
fn TestSideBySideUnwrapped(scope: &mut Scope, content: Arc<diff::DiffContent>) -> Node {
    let scroll_position = use_ref(scope, ScrollPosition::default);
    let content_id = Arc::as_ptr(content) as usize;
    rsx! {
        DiffViewerContainer {
            key: content_id,
            content: Some(Arc::clone(content)),
            colours: Arc::default(),
            viewer_mode: file_types::ViewerMode::SideBySide,
            scroll_position,
            wrap: false,
            compact: false,
            auto_focus: false,
        }
    }
}

fn make_diff(original: &[&str], modified: &[&str]) -> diff::Diff {
    let diff = diff::compute(original, modified, diff::Settings::default()).expect("a diff");
    let alignment = diff::align(diff, original, modified).expect("alignment");
    let file = file_types::File::unchanged_path(
        file_types::RepoPath::new("test.rs", std::path::Path::new("/repo")),
        file_types::Revs::worktree_against(file_types::CommitHash::new("abc")),
    );
    diff::Diff { file, alignment }
}

fn harness_with_colours(
    original: &[&str],
    modified: &[&str],
    width: u16,
    height: u16,
    colours: Arc<Mutex<[Vec<Vec<Span>>; 2]>>,
) -> Harness {
    let content = Arc::new(diff::DiffContent::Diff(make_diff(original, modified)));
    Harness::new::<TestSideBySide>(TestSideBySideProps { content, colours }, width, height)
        .provide::<Ui>(Context {
            theme: Rc::new(Theme::DARK),
            ..Context::default()
        })
}

fn harness(original: &[&str], modified: &[&str], width: u16, height: u16) -> Harness {
    harness_with_colours(original, modified, width, height, Arc::default())
}

fn render(original: &[&str], modified: &[&str], width: u16, height: u16) -> Vec<String> {
    harness(original, modified, width, height).screen()
}

fn harness_unwrapped(original: &[&str], modified: &[&str], width: u16, height: u16) -> Harness {
    let content = Arc::new(diff::DiffContent::Diff(make_diff(original, modified)));
    let mut harness = Harness::new::<TestSideBySideUnwrapped>(
        TestSideBySideUnwrappedProps { content },
        width,
        height,
    )
    .provide::<Ui>(Context {
        theme: Rc::new(Theme::DARK),
        ..Context::default()
    });
    harness.force_draw().force_draw();
    harness
}

fn symbols(harness: &mut Harness, start: u16, end: u16) -> String {
    let cells = harness.cells();
    (start..end)
        .filter_map(|x| cells.cell((x, 0)))
        .map(|cell| cell.symbol())
        .collect()
}

#[test]
fn unchanged_lines_appear_on_both_sides() {
    let lines = render(&["hello", "world"], &["hello", "world"], 40, 4);
    assert!(lines[0].contains("hello"), "left has hello: {:?}", lines[0]);
    let has_two = lines[0].matches("hello").count();
    assert_eq!(has_two, 2, "hello appears on both sides: {:?}", lines[0]);
}

#[test]
fn a_deleted_line_shows_filler_on_the_right() {
    let lines = render(&["removed", "kept"], &["kept"], 40, 4);
    let filler_line = lines.iter().find(|line| line.contains('╱'));
    assert!(filler_line.is_some(), "a filler appears: {:?}", lines);
}

#[test]
fn an_inserted_line_shows_filler_on_the_left() {
    let lines = render(&["kept"], &["kept", "added"], 40, 4);
    let filler_line = lines.iter().find(|line| line.contains('╱'));
    assert!(filler_line.is_some(), "a filler appears: {:?}", lines);
}

#[test]
fn line_numbers_are_drawn() {
    let lines = render(&["one", "two", "three"], &["one", "two", "three"], 40, 5);
    assert!(lines[0].contains('1'), "line 1: {:?}", lines[0]);
    assert!(lines[1].contains('2'), "line 2: {:?}", lines[1]);
}

#[test]
fn a_divider_separates_the_two_sides() {
    let lines = render(&["a"], &["a"], 40, 3);
    assert!(lines[0].contains('│'), "a divider: {:?}", lines[0]);
}

#[test]
fn syntax_service_colours_both_sides() {
    let colours = highlighted(diff::DiffContent::Diff(make_diff(
        &["fn before() {}"],
        &["fn after() {}"],
    )));
    let mut harness = harness_with_colours(&["fn before() {}"], &["fn after() {}"], 40, 2, colours);
    harness.force_draw().force_draw();

    let divider = (0..40)
        .find(|&x| {
            harness
                .cells()
                .cell((x, 0))
                .is_some_and(|cell| cell.symbol() == "│")
        })
        .unwrap();
    assert_ne!(harness.style_at(4, 0).fg, Theme::DARK.normal.fg);
    assert_ne!(harness.style_at(divider + 5, 0).fg, Theme::DARK.normal.fg);
}

#[test]
fn horizontal_input_does_not_move_wrapped_text() {
    let mut harness = harness(&["ABCDEFGHIJKLMNOPQRST"], &["abcdefghijklmnopqrst"], 25, 2);
    let before = harness.screen();

    harness
        .press(crokey::key!(l))
        .wheel_horizontal(10, 1, 1)
        .press(crokey::key!('$'))
        .force_draw();

    assert_eq!(harness.screen(), before);
}

#[test]
fn an_odd_text_cell_still_leaves_the_divider_in_place() {
    let mut harness = harness(&["ABCDEFGHIJKLMNOPQRST"], &["abcdefghijklmnopqrst"], 26, 2);
    harness.force_draw().force_draw();

    assert_eq!(symbols(&mut harness, 13, 14), "│");
    assert!(symbols(&mut harness, 4, 13).starts_with("ABCDEFGHI"));
    assert!(symbols(&mut harness, 18, 26).starts_with("abcdefgh"));
}

#[test]
fn the_shorter_side_starts_with_its_first_fragment() {
    let mut harness = harness(&["ABCDEFGHIJKL"], &["abcdefghijklmnopqrst"], 25, 2);
    harness.press(crokey::key!('$')).force_draw();

    assert!(symbols(&mut harness, 4, 12).starts_with("ABCDEFGH"));
    assert!(symbols(&mut harness, 17, 25).starts_with("abcdefgh"));
}

#[test]
fn the_modified_side_starts_with_its_first_fragment() {
    let mut harness = harness(&["ABCDEFGHIJKLMNOPQRST"], &["abcdefghijkl"], 25, 2);
    harness.press(crokey::key!('$')).force_draw();

    assert!(symbols(&mut harness, 4, 12).starts_with("ABCDEFGH"));
    assert!(symbols(&mut harness, 17, 25).starts_with("abcdefgh"));
}

#[test]
fn long_lines_render_continuations_on_both_sides() {
    let original = format!(
        "SIDE_WRAP_ORIGINAL_START {} SIDE_WRAP_ORIGINAL_END",
        "0123456789 ".repeat(4)
    );
    let modified = format!(
        "SIDE_WRAP_MODIFIED_START {} SIDE_WRAP_MODIFIED_END",
        "abcdefghij ".repeat(4)
    );
    let original = [original.as_str()];
    let modified = [modified.as_str()];
    let mut harness = harness(&original, &modified, 60, 8);
    let screen = harness.screen();

    assert!(
        screen
            .iter()
            .skip(1)
            .any(|line| line.contains("SIDE_WRAP_ORIGINAL_END")),
        "original continuation is missing: {screen:?}"
    );
    assert!(
        screen
            .iter()
            .skip(1)
            .any(|line| line.contains("SIDE_WRAP_MODIFIED_END")),
        "modified continuation is missing: {screen:?}"
    );
}

#[test]
fn unwrapped_long_lines_scroll_horizontally_instead_of_wrapping() {
    let original = "SIDE_UNWRAPPED_ORIGINAL ".to_owned() + &"0123456789".repeat(8);
    let modified = "SIDE_UNWRAPPED_MODIFIED ".to_owned() + &"abcdefghij".repeat(8);
    let mut harness = harness_unwrapped(&[original.as_str()], &[modified.as_str()], 44, 4);
    let before = harness.screen();

    harness.press(crokey::key!('$')).force_draw();

    assert_ne!(harness.screen(), before);
    assert!(harness.screen().iter().any(|line| line.contains("hij")));
}

#[test]
fn a_wrapped_fragment_keeps_its_side_style_and_opposite_filler() {
    let mut harness = harness(&["abcdefghijkl"], &["ABC"], 25, 2);
    harness.force_draw().force_draw();

    assert_eq!(harness.screen_line(1), "    ijkl    │    ╱╱╱╱╱╱╱╱");
    assert_eq!(
        harness.style_at(4, 1).bg,
        Theme::DARK.normal.patch(Theme::DARK.deleted_text).bg
    );
    assert_eq!(
        harness.style_at(17, 0).bg,
        Theme::DARK.normal.patch(Theme::DARK.inserted_text).bg
    );
    assert_eq!(
        harness.style_at(17, 1).bg,
        Theme::DARK.normal.patch(Theme::DARK.filler).bg
    );
}

#[test]
fn j_scrolls_one_line_immediately() {
    let lines: Vec<String> = (1..=20).map(|line| format!("line {line}")).collect();
    let lines: Vec<&str> = lines.iter().map(String::as_str).collect();
    let mut h = harness(&lines, &lines, 40, 4);
    h.force_draw().force_draw();
    let before = h.screen();

    h.press(crokey::key!(j)).force_draw();

    assert_ne!(h.screen(), before);
}

#[test]
fn the_wheel_scrolls_without_a_keypress() {
    let lines: Vec<String> = (1..=20).map(|line| format!("line {line}")).collect();
    let lines: Vec<&str> = lines.iter().map(String::as_str).collect();
    let mut h = harness(&lines, &lines, 40, 4);
    h.force_draw().force_draw();
    let before = h.screen();

    h.wheel(10, 1, 1).force_draw();

    assert_ne!(h.screen(), before);
}
