//! Tests for SideBySide.

use std::cell::RefCell;
use std::rc::Rc;
use std::sync::mpsc;
use std::time::Duration;

use loom::testing::Harness;
use ui::Theme;
use ui::components::side_by_side::{SideBySide, SideBySideProps};
use ui::components::{Context, Ui};
use ui::services::syntax::SyntaxService;

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
    Harness::new::<SideBySide>(SideBySideProps { content }, width, height).provide::<Ui>(Context {
        theme: Rc::new(Theme::DARK),
        syntax_service,
        ..Context::default()
    })
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
fn horizontal_scroll_keeps_gutters_and_divider_fixed() {
    let mut harness = harness(&["ABCDEFGHIJKLMNOPQRST"], &["abcdefghijklmnopqrst"], 25, 2);
    harness.force_draw().force_draw();
    let left_gutter = symbols(&mut harness, 0, 4);
    let divider = symbols(&mut harness, 12, 13);
    let right_gutter = symbols(&mut harness, 13, 17);

    for _ in 0..3 {
        harness.press(crokey::key!(l)).force_draw();
    }

    assert_eq!(symbols(&mut harness, 0, 4), left_gutter);
    assert_eq!(symbols(&mut harness, 12, 13), divider);
    assert_eq!(symbols(&mut harness, 13, 17), right_gutter);
    assert_eq!(symbols(&mut harness, 4, 12), "DEFGHIJK");
    assert_eq!(symbols(&mut harness, 17, 25), "defghijk");
}

#[test]
fn an_odd_text_cell_goes_to_the_original_side() {
    let mut harness = harness(&["ABCDEFGHIJKLMNOPQRST"], &["abcdefghijklmnopqrst"], 26, 2);
    harness.force_draw().force_draw();

    assert_eq!(symbols(&mut harness, 4, 13), "ABCDEFGHI");
    assert_eq!(symbols(&mut harness, 13, 14), "│");
    assert_eq!(symbols(&mut harness, 18, 26), "abcdefgh");

    for _ in 0..20 {
        harness.press(crokey::key!(l)).force_draw();
    }
    assert_eq!(symbols(&mut harness, 4, 13), "PQRST    ");
    assert_eq!(symbols(&mut harness, 18, 26), "qrst    ");
}

#[test]
fn the_shorter_side_stops_while_the_longer_side_continues() {
    let mut harness = harness(&["ABCDEFGHIJKL"], &["abcdefghijklmnopqrst"], 25, 2);
    harness.force_draw().force_draw();

    for _ in 0..10 {
        harness.press(crokey::key!(l)).force_draw();
    }

    assert_eq!(symbols(&mut harness, 4, 12), "IJKL    ");
    assert_eq!(symbols(&mut harness, 17, 25), "klmnopqr");
}

#[test]
fn the_modified_side_can_stop_before_the_original() {
    let mut harness = harness(&["ABCDEFGHIJKLMNOPQRST"], &["abcdefghijkl"], 25, 2);
    harness.force_draw().force_draw();

    for _ in 0..10 {
        harness.press(crokey::key!(l)).force_draw();
    }

    assert_eq!(symbols(&mut harness, 4, 12), "KLMNOPQR");
    assert_eq!(symbols(&mut harness, 17, 25), "ijkl    ");
}

#[test]
fn the_endpoint_keeps_four_cells_after_each_longest_line() {
    let mut harness = harness(&["ABCDEFGHIJKL"], &["abcdefghijklmnopqrst"], 25, 2);
    harness.force_draw().force_draw();

    for _ in 0..20 {
        harness.press(crokey::key!(l)).force_draw();
    }

    assert_eq!(symbols(&mut harness, 4, 12), "IJKL    ");
    assert_eq!(symbols(&mut harness, 17, 25), "qrst    ");
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
