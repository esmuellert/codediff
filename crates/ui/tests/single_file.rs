use std::cell::RefCell;
use std::path::Path;
use std::rc::Rc;
use std::sync::Arc;
use std::sync::mpsc;
use std::time::Duration;

use loom::testing::Harness;
use ui::Theme;
use ui::components::single_file::{SingleFile, SingleFileProps};
use ui::components::{Context, Ui};
use ui::services::syntax::SyntaxService;

fn file(deleted: bool) -> file_types::File {
    named_file("plain.rs", deleted)
}

fn named_file(path: &str, deleted: bool) -> file_types::File {
    let path = file_types::RepoPath::new(path, Path::new("/repo"));
    let revs = file_types::Revs::worktree_against(file_types::Oid::new("abc"));
    if deleted {
        file_types::File::deleted(path, revs)
    } else {
        file_types::File::added(path, revs)
    }
}

fn harness_with_syntax_service(
    lines: Vec<String>,
    deleted: bool,
    width: u16,
    height: u16,
    syntax_service: Option<Rc<SyntaxService>>,
) -> Harness {
    let file = file(deleted);
    let content = Rc::new(pipeline::diff::DiffContent::SingleFile(
        pipeline::diff::SingleFile {
            file,
            lines: Arc::new(lines),
        },
    ));
    Harness::new::<SingleFile>(
        SingleFileProps {
            content,
            wrap: true,
        },
        width,
        height,
    )
    .provide::<Ui>(Context {
        theme: Rc::new(Theme::DARK),
        syntax_service,
        ..Context::default()
    })
}

fn harness(lines: Vec<String>, deleted: bool, width: u16, height: u16) -> Harness {
    harness_with_syntax_service(lines, deleted, width, height, None)
}

fn harness_unwrapped(lines: Vec<String>, deleted: bool, width: u16, height: u16) -> Harness {
    let file = file(deleted);
    let content = Rc::new(pipeline::diff::DiffContent::SingleFile(
        pipeline::diff::SingleFile {
            file,
            lines: Arc::new(lines),
        },
    ));
    let mut harness = Harness::new::<SingleFile>(
        SingleFileProps {
            content,
            wrap: false,
        },
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
fn a_long_line_renders_its_continuation() {
    let line = format!(
        "SINGLE_WRAP_START {} SINGLE_WRAP_END",
        "0123456789 ".repeat(4)
    );
    let mut harness = harness(vec![line], false, 32, 8);

    assert!(
        harness
            .screen()
            .iter()
            .skip(1)
            .any(|screen_line| screen_line.contains("SINGLE_WRAP_END")),
        "wrapped continuation is missing: {:?}",
        harness.screen()
    );
}

#[test]
fn unwrapped_long_lines_scroll_horizontally_instead_of_wrapping() {
    let line = "SINGLE_UNWRAPPED ".to_owned() + &"0123456789".repeat(8);
    let mut harness = harness_unwrapped(vec![line], false, 24, 4);
    let before = harness.screen();

    harness.press(crokey::key!('$')).force_draw();

    assert_ne!(harness.screen(), before);
    assert!(harness.screen().iter().any(|row| row.contains("789")));
    assert_eq!(harness.screen().len(), 4);
}

#[test]
fn toggling_wrap_preserves_the_current_terminal_line() {
    let long = "SINGLE_TOGGLE ".to_owned() + &"0123456789".repeat(12);
    let content = Rc::new(pipeline::diff::DiffContent::SingleFile(
        pipeline::diff::SingleFile {
            file: named_file("toggle.rs", false),
            lines: Arc::new(vec![long, "second".into(), "third".into()]),
        },
    ));
    let mut harness = Harness::new::<SingleFile>(
        SingleFileProps {
            content: Rc::clone(&content),
            wrap: true,
        },
        32,
        2,
    )
    .provide::<Ui>(Context {
        theme: Rc::new(Theme::DARK),
        ..Context::default()
    });
    harness.force_draw().force_draw();
    harness
        .press(crokey::key!(j))
        .press(crokey::key!(j))
        .force_draw();

    harness.set_props::<SingleFile>(SingleFileProps {
        content,
        wrap: false,
    });
    harness.force_draw().force_draw();

    assert!(harness.screen_row(0).contains("SINGLE_TOGGLE"));
}

#[test]
fn lines_are_numbered_in_one_full_width_pane() {
    let mut harness = harness(vec!["alpha".into(), "beta".into()], false, 30, 3);
    let screen = harness.screen();

    assert!(screen[0].contains("1 alpha"), "got {screen:?}");
    assert!(screen[1].contains("2 beta"), "got {screen:?}");
    assert_eq!(screen[0].matches("alpha").count(), 1);
    assert!(!screen.iter().any(|row| row.contains(['│', '╱'])));
}

#[test]
fn added_and_deleted_files_have_no_diff_background() {
    for deleted in [false, true] {
        let mut harness = harness(vec!["plain".into()], deleted, 20, 2);
        harness.force_draw().force_draw();

        let style = harness.style_at(4, 0);
        assert_eq!(style.fg, Theme::DARK.normal.fg);
        assert_eq!(style.bg, Theme::DARK.normal.bg);
        assert_ne!(
            style.bg,
            Theme::DARK.normal.patch(Theme::DARK.inserted_text).bg
        );
        assert_ne!(
            style.bg,
            Theme::DARK.normal.patch(Theme::DARK.deleted_text).bg
        );
    }
}

#[test]
fn syntax_is_requested_for_the_present_side() {
    let (syntax_tx, syntax_responses) = mpsc::channel();
    let syntax_worker =
        syntax::Syntax::start(channel::Emitter::new(syntax_tx, |response| response));
    let syntax_service = Rc::new(SyntaxService::new(Rc::new(RefCell::new(syntax_worker))));
    let mut harness = harness_with_syntax_service(
        vec!["fn main() {}".into()],
        false,
        30,
        2,
        Some(Rc::clone(&syntax_service)),
    );
    harness.force_draw().force_draw();
    let response = syntax_responses
        .recv_timeout(Duration::from_secs(1))
        .expect("syntax response");
    syntax_service.deliver(response);
    harness.force_draw().force_draw();

    assert_ne!(harness.style_at(4, 0).fg, Theme::DARK.normal.fg);
    assert_eq!(harness.style_at(4, 0).bg, Theme::DARK.normal.bg);
}

#[test]
fn horizontal_input_does_not_move_wrapped_text() {
    let mut harness = harness(vec!["ABCDEFGHIJKL".into()], false, 12, 2);
    let before = harness.screen();

    harness
        .press(crokey::key!(l))
        .wheel_horizontal(10, 1, 1)
        .press(crokey::key!('$'))
        .force_draw();

    assert_eq!(harness.screen(), before);
}

#[test]
fn an_offscreen_longest_line_stays_at_the_start() {
    let mut harness = harness(vec!["short".into(), "ABCDEFGHIJKL".into()], false, 12, 1);
    harness.press(crokey::key!('$')).force_draw();
    assert!(symbols(&mut harness, 4, 12).starts_with("short"));
}

#[test]
fn a_long_file_scrolls() {
    let lines = (1..=20).map(|line| format!("line {line}")).collect();
    let mut harness = harness(lines, false, 30, 4);
    harness.force_draw().force_draw();
    let before = harness.screen();

    for _ in 0..8 {
        harness.press(crokey::key!(j)).force_draw();
    }

    assert_ne!(harness.screen(), before);
}

#[test]
fn each_file_restores_its_position() {
    let first_lines: Vec<String> = (1..=12).map(|line| format!("first {line:02}")).collect();
    let second_lines: Vec<String> = (1..=12).map(|line| format!("second {line:02}")).collect();
    let first = Rc::new(pipeline::diff::DiffContent::SingleFile(
        pipeline::diff::SingleFile {
            file: named_file("first.rs", false),
            lines: Arc::new(first_lines),
        },
    ));
    let second = Rc::new(pipeline::diff::DiffContent::SingleFile(
        pipeline::diff::SingleFile {
            file: named_file("second.rs", false),
            lines: Arc::new(second_lines),
        },
    ));
    let mut harness = Harness::new::<SingleFile>(
        SingleFileProps {
            content: first.clone(),
            wrap: true,
        },
        30,
        4,
    )
    .provide::<Ui>(Context {
        theme: Rc::new(Theme::DARK),
        ..Context::default()
    });
    harness.force_draw().force_draw();
    for _ in 0..3 {
        harness.press(crokey::key!(j)).force_draw();
    }
    assert!(harness.screen_row(0).contains("first 04"));

    harness.set_props::<SingleFile>(SingleFileProps {
        content: second.clone(),
        wrap: true,
    });
    harness.force_draw().force_draw();
    assert!(harness.screen_row(0).contains("second 01"));
    harness.press(crokey::key!(j)).force_draw();

    harness.set_props::<SingleFile>(SingleFileProps {
        content: first,
        wrap: true,
    });
    harness.force_draw().force_draw();
    assert!(harness.screen_row(0).contains("first 04"));
}
