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
    let path = file_types::RepoPath::new("plain.rs", Path::new("/repo"));
    let revs = file_types::Revs::worktree_against(file_types::Oid::new("abc"));
    if deleted {
        file_types::File::deleted(path, revs)
    } else {
        file_types::File::added(path, revs)
    }
}

fn harness_with_service(
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
    Harness::new::<SingleFile>(SingleFileProps { content }, width, height).provide::<Ui>(Context {
        theme: Rc::new(Theme::DARK),
        syntax_service,
        ..Context::default()
    })
}

fn harness(lines: Vec<String>, deleted: bool, width: u16, height: u16) -> Harness {
    harness_with_service(lines, deleted, width, height, None)
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
    let (tx, rx) = mpsc::channel();
    let worker = syntax::Syntax::start(channel::Emitter::new(tx, |response| response));
    let syntax_service = Rc::new(SyntaxService::new(Rc::new(RefCell::new(worker))));
    let mut harness = harness_with_service(
        vec!["fn main() {}".into()],
        false,
        30,
        2,
        Some(Rc::clone(&syntax_service)),
    );
    harness.force_draw().force_draw();
    let response = rx
        .recv_timeout(Duration::from_secs(1))
        .expect("syntax response");
    syntax_service.deliver(response);
    harness.force_draw().force_draw();

    assert_ne!(harness.style_at(4, 0).fg, Theme::DARK.normal.fg);
    assert_eq!(harness.style_at(4, 0).bg, Theme::DARK.normal.bg);
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
