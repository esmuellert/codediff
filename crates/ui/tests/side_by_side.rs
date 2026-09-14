//! Tests for SideBySide.

use std::cell::RefCell;
use std::rc::Rc;
use std::sync::mpsc;
use std::time::Duration;

use loom::testing::Harness;
use loom::{Node, Scope, component, rsx, use_ref};
use ui::Theme;
use ui::components::diff_viewer::{ViewState, ViewStateHistory};
use ui::components::side_by_side::{SideBySide, SideBySideProps};
use ui::components::{Context, Ui};
use ui::services::syntax::SyntaxService;

#[component]
fn TestSideBySide(scope: &mut Scope, content: Rc<pipeline::diff::DiffContent>) -> Node {
    let view_states = use_ref(scope, ViewStateHistory::default);
    let content_id = Rc::as_ptr(content) as usize;
    let key = content.file().path().as_str().to_owned();
    let active_view_state = use_ref(scope, ViewState::default);
    let active_key = use_ref(scope, || None::<String>);
    let previous_key = active_key.current().clone();
    if previous_key.as_deref() != Some(key.as_str()) {
        if let Some(previous_key) = previous_key {
            let state = active_view_state.current().clone();
            view_states.current().save(&previous_key, state);
        }
        *active_view_state.current() = view_states.current().load(&key);
        *active_key.current() = Some(key);
    }
    rsx! {
        SideBySide {
            key: content_id,
            content: Rc::clone(content),
            view_state: active_view_state,
            wrap: true,
            auto_focus: false,
        }
    }
}

fn make_diff(original: &[&str], modified: &[&str]) -> pipeline::diff::Diff {
    let diff = pipeline::diff::compute(original, modified).expect("a diff");
    let alignment = pipeline::diff::align(diff, original, modified).expect("alignment");
    let file = file_types::File::unchanged_path(
        file_types::RepoPath::new("test.rs", std::path::Path::new("/repo")),
        file_types::Revs::worktree_against(file_types::Oid::new("abc")),
    );
    pipeline::diff::Diff { file, alignment }
}

fn harness_with_syntax_service(
    original: &[&str],
    modified: &[&str],
    width: u16,
    height: u16,
    syntax_service: Option<Rc<SyntaxService>>,
) -> Harness {
    let content = Rc::new(pipeline::diff::DiffContent::Diff(make_diff(
        original, modified,
    )));
    Harness::new::<TestSideBySide>(TestSideBySideProps { content }, width, height).provide::<Ui>(
        Context {
            theme: Rc::new(Theme::DARK),
            syntax_service,
            ..Context::default()
        },
    )
}

fn harness(original: &[&str], modified: &[&str], width: u16, height: u16) -> Harness {
    harness_with_syntax_service(original, modified, width, height, None)
}

fn render(original: &[&str], modified: &[&str], width: u16, height: u16) -> Vec<String> {
    harness(original, modified, width, height).screen()
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
    let rows = render(&["hello", "world"], &["hello", "world"], 40, 4);
    assert!(rows[0].contains("hello"), "left has hello: {:?}", rows[0]);
    let has_two = rows[0].matches("hello").count();
    assert_eq!(has_two, 2, "hello appears on both sides: {:?}", rows[0]);
}

#[test]
fn a_deleted_line_shows_filler_on_the_right() {
    let rows = render(&["removed", "kept"], &["kept"], 40, 4);
    let filler_row = rows.iter().find(|r| r.contains('╱'));
    assert!(filler_row.is_some(), "a filler appears: {:?}", rows);
}

#[test]
fn an_inserted_line_shows_filler_on_the_left() {
    let rows = render(&["kept"], &["kept", "added"], 40, 4);
    let filler_row = rows.iter().find(|r| r.contains('╱'));
    assert!(filler_row.is_some(), "a filler appears: {:?}", rows);
}

#[test]
fn line_numbers_are_drawn() {
    let rows = render(&["one", "two", "three"], &["one", "two", "three"], 40, 5);
    assert!(rows[0].contains('1'), "line 1: {:?}", rows[0]);
    assert!(rows[1].contains('2'), "line 2: {:?}", rows[1]);
}

#[test]
fn a_divider_separates_the_two_sides() {
    let rows = render(&["a"], &["a"], 40, 3);
    assert!(rows[0].contains('│'), "a divider: {:?}", rows[0]);
}

#[test]
fn syntax_is_requested_for_both_sides() {
    let (syntax_tx, syntax_responses) = mpsc::channel();
    let syntax_worker =
        syntax::Syntax::start(channel::Emitter::new(syntax_tx, |response| response));
    let syntax_service = Rc::new(SyntaxService::new(Rc::new(RefCell::new(syntax_worker))));
    let mut harness = harness_with_syntax_service(
        &["fn before() {}"],
        &["fn after() {}"],
        40,
        2,
        Some(Rc::clone(&syntax_service)),
    );
    harness.force_draw().force_draw();
    for _ in 0..2 {
        let response = syntax_responses
            .recv_timeout(Duration::from_secs(1))
            .expect("syntax response");
        syntax_service.deliver(response);
    }
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
