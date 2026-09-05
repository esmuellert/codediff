//! Tests for the render-only Inline component.

use std::cell::RefCell;
use std::rc::Rc;
use std::sync::mpsc;
use std::time::Duration;

use loom::testing::Harness;
use ui::Theme;
use ui::components::inline::{Inline, InlineProps};
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
    Harness::new::<Inline>(InlineProps { content }, width, height).provide::<Ui>(Context {
        theme: Rc::new(Theme::DARK),
        syntax_service,
        ..Context::default()
    })
}

fn harness(original: &[&str], modified: &[&str], width: u16, height: u16) -> Harness {
    let mut harness = harness_with_syntax_service(original, modified, width, height, None);
    for _ in 0..4 {
        harness.force_draw();
    }
    harness
}

#[test]
fn changes_read_original_then_modified_in_one_text_column() {
    let mut harness = harness(
        &["one", "before", "three"],
        &["one", "after", "extra", "three"],
        40,
        6,
    );
    let rows = harness.screen();

    assert!(rows[0].starts_with("  1   1 one"), "{:?}", rows[0]);
    assert!(rows[1].starts_with("  2     before"), "{:?}", rows[1]);
    assert!(rows[2].starts_with("      2 after"), "{:?}", rows[2]);
    assert!(rows[3].starts_with("      3 extra"), "{:?}", rows[3]);
    assert!(rows[4].starts_with("  3   4 three"), "{:?}", rows[4]);
    assert!(!rows.iter().any(|row| row.contains(['│', '╱'])));
}

#[test]
fn changed_rows_colour_both_gutters_and_the_text_column() {
    let mut harness = harness(&["before shared"], &["after shared"], 30, 3);
    let deleted = Theme::DARK.normal.patch(Theme::DARK.deleted).bg;
    let inserted = Theme::DARK.normal.patch(Theme::DARK.inserted).bg;

    for x in [0, 4, 20] {
        assert_eq!(harness.style_at(x, 0).bg, deleted, "deleted cell {x}");
        assert_eq!(harness.style_at(x, 1).bg, inserted, "inserted cell {x}");
    }
}

#[test]
fn empty_and_nonempty_character_ranges_reach_code_text() {
    let mut harness = harness(&["abc"], &["axbc"], 30, 3);
    let deleted_text = Theme::DARK.normal.patch(Theme::DARK.deleted_text).bg;
    let inserted_text = Theme::DARK.normal.patch(Theme::DARK.inserted_text).bg;
    let code_start = 8;

    assert_eq!(
        harness.style_at(code_start + 1, 0).underline_color,
        deleted_text
    );
    assert_eq!(harness.style_at(code_start + 1, 1).bg, inserted_text);
}

#[test]
fn syntax_is_requested_for_both_versions() {
    let (syntax_tx, syntax_responses) = mpsc::channel();
    let syntax_worker =
        syntax::Syntax::start(channel::Emitter::new(syntax_tx, |response| response));
    let syntax_service = Rc::new(SyntaxService::new(Rc::new(RefCell::new(syntax_worker))));
    let mut harness = harness_with_syntax_service(
        &["fn before() {}"],
        &["fn after() {}"],
        40,
        3,
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

    assert_ne!(harness.style_at(8, 0).fg, Theme::DARK.normal.fg);
    assert_ne!(harness.style_at(8, 1).fg, Theme::DARK.normal.fg);
}

#[test]
fn input_does_not_move_the_render_only_component() {
    let lines: Vec<String> = (1..=20)
        .map(|line| format!("line {line:02} abcdefghijklmnop"))
        .collect();
    let lines: Vec<&str> = lines.iter().map(String::as_str).collect();
    let mut harness = harness(&lines, &lines, 20, 4);
    let before = harness.screen();

    harness
        .press(crokey::key!(j))
        .press(crokey::key!(l))
        .wheel(10, 1, 1)
        .wheel_horizontal(10, 1, 1)
        .force_draw();

    assert_eq!(harness.screen(), before);
}
