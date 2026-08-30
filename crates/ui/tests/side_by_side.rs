//! Tests for SideBySide.

use std::rc::Rc;

use loom::testing::Harness;
use ui::Theme;
use ui::components::side_by_side::{SideBySide, SideBySideProps};
use ui::components::{Context, Ui};

fn make_diff(original: &[&str], modified: &[&str]) -> pipeline::diff::Diff {
    let diff = pipeline::diff::diff::compute(original, modified).expect("a diff");
    let alignment = pipeline::diff::diff::align(diff, original, modified).expect("alignment");
    let file = file_types::File::unchanged_path(
        file_types::RepoPath::new("test.rs", std::path::Path::new("/repo")),
        file_types::Revs::worktree_against(file_types::Oid::new("abc")),
    );
    pipeline::diff::Diff { file, alignment }
}

fn render(original: &[&str], modified: &[&str], width: u16, height: u16) -> Vec<String> {
    let diff = make_diff(original, modified);
    let content = pipeline::diff::DiffContent::Diff(diff);
    let mut h = Harness::new::<SideBySide>(
        SideBySideProps {},
        width, height,
    ).provide::<Ui>(Context {
        theme: Rc::new(Theme::DARK),
        diff: Some(Rc::new(content)),
        ..Context::default()
    });
    h.screen()
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
