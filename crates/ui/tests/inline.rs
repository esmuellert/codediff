//! Tests for the Inline component.

use std::cell::RefCell;
use std::rc::Rc;
use std::sync::mpsc;
use std::time::Duration;

use loom::testing::Harness;
use loom::{Node, Scope, component, rsx, use_ref};
use ui::Theme;
use ui::components::diff_viewer::{ViewState, ViewStateHistory};
use ui::components::inline::{Inline, InlineProps};
use ui::components::{Context, Ui};
use ui::services::syntax::SyntaxService;

#[component]
fn TestInline(scope: &mut Scope, content: Rc<pipeline::diff::DiffContent>) -> Node {
    let view_states = use_ref(scope, ViewStateHistory::default);
    let content_id = Rc::as_ptr(content) as usize;
    let key = content.file().path().as_str().to_owned();
    let active_view_state = use_ref(scope, ViewState::default);
    let active_key = use_ref(scope, || None::<String>);
    let previous_key = active_key.current().clone();
    if previous_key.as_deref() != Some(key.as_str()) {
        if let Some(previous_key) = previous_key {
            let state = *active_view_state.current();
            view_states.current().save(&previous_key, state);
        }
        *active_view_state.current() = view_states.current().load(&key);
        *active_key.current() = Some(key);
    }
    rsx! {
        Inline {
            key: content_id,
            content: Rc::clone(content),
            view_state: active_view_state,
            auto_focus: false,
        }
    }
}

fn make_diff(path: &str, original: &[&str], modified: &[&str]) -> pipeline::diff::Diff {
    let diff = pipeline::diff::compute(original, modified).expect("a diff");
    let alignment = pipeline::diff::align(diff, original, modified).expect("alignment");
    let file = file_types::File::unchanged_path(
        file_types::RepoPath::new(path, std::path::Path::new("/repo")),
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
        "test.rs", original, modified,
    )));
    Harness::new::<TestInline>(TestInlineProps { content }, width, height).provide::<Ui>(Context {
        theme: Rc::new(Theme::DARK),
        syntax_service,
        ..Context::default()
    })
}

fn harness(original: &[&str], modified: &[&str], width: u16, height: u16) -> Harness {
    let mut harness = harness_with_syntax_service(original, modified, width, height, None);
    settle(&mut harness);
    harness
}

fn settle(harness: &mut Harness) {
    for _ in 0..4 {
        harness.force_draw();
    }
}

fn symbols(harness: &mut Harness, start: u16, end: u16, row: u16) -> String {
    let cells = harness.cells();
    (start..end)
        .filter_map(|column| cells.cell((column, row)))
        .map(|cell| cell.symbol())
        .collect()
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
fn vertical_keys_and_wheel_move_visual_rows() {
    let lines: Vec<String> = (1..=20).map(|line| format!("line {line:02}")).collect();
    let lines: Vec<&str> = lines.iter().map(String::as_str).collect();
    let mut harness = harness(&lines, &lines, 30, 4);

    assert!(harness.screen_row(0).contains("line 01"));
    for _ in 0..4 {
        harness.press(crokey::key!(j));
    }
    harness.force_draw();
    assert!(harness.screen_row(0).contains("line 05"));

    harness.wheel(1, 1, 1).force_draw();
    assert!(harness.screen_row(0).contains("line 08"));

    harness.wheel(1, 1, -1).force_draw();
    assert!(harness.screen_row(0).contains("line 05"));
    harness.press(crokey::key!(k)).force_draw();
    assert!(harness.screen_row(0).contains("line 04"));
    harness.press(crokey::key!(down)).force_draw();
    assert!(harness.screen_row(0).contains("line 05"));
    harness.press(crokey::key!(up)).force_draw();
    assert!(harness.screen_row(0).contains("line 04"));
}

#[test]
fn horizontal_keys_and_wheel_keep_both_gutters_fixed() {
    let mut harness = harness(&["ABCDEFGHIJKLMNOPQRST"], &["abcdefghijklmnopqrst"], 20, 3);
    let original_row_gutters = symbols(&mut harness, 0, 8, 0);
    let modified_row_gutters = symbols(&mut harness, 0, 8, 1);

    for _ in 0..3 {
        harness.press(crokey::key!(l));
    }
    harness.force_draw();
    assert_eq!(symbols(&mut harness, 0, 8, 0), original_row_gutters);
    assert_eq!(symbols(&mut harness, 0, 8, 1), modified_row_gutters);
    assert_eq!(symbols(&mut harness, 8, 20, 0), "DEFGHIJKLMNO");
    assert_eq!(symbols(&mut harness, 8, 20, 1), "defghijklmno");

    harness.wheel_horizontal(1, 1, 1).force_draw();
    assert_eq!(symbols(&mut harness, 8, 20, 0), "GHIJKLMNOPQR");
    assert_eq!(symbols(&mut harness, 8, 20, 1), "ghijklmnopqr");

    harness.wheel_horizontal(1, 1, -1).force_draw();
    harness.press(crokey::key!(h)).force_draw();
    assert_eq!(symbols(&mut harness, 8, 20, 0), "CDEFGHIJKLMN");
    harness.press(crokey::key!(0)).force_draw();
    assert_eq!(symbols(&mut harness, 8, 20, 0), "ABCDEFGHIJKL");
}

#[test]
fn horizontal_endpoint_leaves_four_cells_and_survives_resize() {
    let mut harness = harness(&["ABCDEFGHIJKLMNOPQRST"], &["abcdefghijklmnopqrst"], 20, 3);

    harness.press(crokey::key!('$')).force_draw();
    assert_eq!(symbols(&mut harness, 8, 20, 0), "MNOPQRST    ");
    assert_eq!(symbols(&mut harness, 8, 20, 1), "mnopqrst    ");

    harness.resize(30, 3).force_draw();
    assert_eq!(symbols(&mut harness, 8, 30, 0), "CDEFGHIJKLMNOPQRST    ");
    harness.resize(20, 3).force_draw();
    assert_eq!(symbols(&mut harness, 8, 20, 0), "MNOPQRST    ");
}

#[test]
fn an_offscreen_longest_line_sets_the_horizontal_endpoint() {
    let lines = ["short", "ABCDEFGHIJKLMNOPQRSTUVWXYZ0123456789"];
    let mut harness = harness(&lines, &lines, 20, 1);

    harness.press(crokey::key!('$')).force_draw();

    assert_eq!(symbols(&mut harness, 8, 20, 0), "            ");
}

#[test]
fn both_versions_follow_the_longer_version_endpoint() {
    let mut harness = harness(&["ABCDEFGHIJKL"], &["abcdefghijklmnopqrst"], 20, 3);

    harness.press(crokey::key!('$')).force_draw();

    assert_eq!(symbols(&mut harness, 8, 20, 0), "            ");
    assert_eq!(symbols(&mut harness, 8, 20, 1), "mnopqrst    ");
}

#[test]
fn each_file_restores_its_vertical_position() {
    let first: Vec<String> = (1..=12).map(|line| format!("first {line:02}")).collect();
    let second: Vec<String> = (1..=12).map(|line| format!("second {line:02}")).collect();
    let first_lines: Vec<&str> = first.iter().map(String::as_str).collect();
    let second_lines: Vec<&str> = second.iter().map(String::as_str).collect();
    let first_content = Rc::new(pipeline::diff::DiffContent::Diff(make_diff(
        "first.rs",
        &first_lines,
        &first_lines,
    )));
    let second_content = Rc::new(pipeline::diff::DiffContent::Diff(make_diff(
        "second.rs",
        &second_lines,
        &second_lines,
    )));
    let mut harness = Harness::new::<TestInline>(
        TestInlineProps {
            content: Rc::clone(&first_content),
        },
        30,
        4,
    )
    .provide::<Ui>(Context {
        theme: Rc::new(Theme::DARK),
        ..Context::default()
    });
    settle(&mut harness);
    for _ in 0..4 {
        harness.press(crokey::key!(j));
    }
    harness.force_draw();
    assert!(harness.screen_row(0).contains("first 05"));

    harness.set_props::<TestInline>(TestInlineProps {
        content: Rc::clone(&second_content),
    });
    settle(&mut harness);
    assert!(harness.screen_row(0).contains("second 01"));
    harness.press(crokey::key!(j)).force_draw();
    assert!(harness.screen_row(0).contains("second 02"));

    harness.set_props::<TestInline>(TestInlineProps {
        content: first_content,
    });
    settle(&mut harness);
    assert!(harness.screen_row(0).contains("first 05"));
}

#[test]
fn clicking_the_component_focuses_it() {
    let mut harness = harness(&["same"], &["same"], 20, 2);
    assert_eq!(harness.focused_name(), None);

    harness.click(10, 0).force_draw();

    assert_eq!(harness.focused_name(), Some("Inline"));
}
