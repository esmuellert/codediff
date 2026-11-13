use std::path::Path;
use std::rc::Rc;
use std::sync::{Arc, mpsc};

use loom::testing::Harness;
use loom::{Layout, Node, Row, RowProps, Scope, component, rsx};
use pipeline::Pipeline;
use ui::Theme;
use ui::components::diff_viewer::{DiffViewer, DiffViewerProps};
use ui::components::{Context, Explorer, Ui, UiProps};
use ui::services::watcher::WatcherService;

use crate::fake_pipeline::{fake_pipeline, wait};

fn file(path: &str) -> file_types::File {
    file_types::File::unchanged_path(
        file_types::RepoPath::new(path, Path::new("/repo")),
        file_types::Revs::worktree_against(file_types::Oid::new("abc")),
    )
}

fn make_diff(file: file_types::File) -> pipeline::diff::DiffContent {
    make_diff_with_lines(file, &["hello", "world"], &["hello", "world"])
}

fn make_diff_with_lines(
    file: file_types::File,
    original: &[&str],
    modified: &[&str],
) -> pipeline::diff::DiffContent {
    let diff = pipeline::diff::compute(original, modified, pipeline::diff::Settings::default())
        .expect("a diff");
    let alignment = pipeline::diff::align(diff, original, modified).expect("alignment");
    pipeline::diff::DiffContent::Diff(pipeline::diff::Diff { file, alignment })
}

fn make_single(file: file_types::File) -> pipeline::diff::DiffContent {
    make_single_with_text(file, "untracked body")
}

fn make_single_with_text(file: file_types::File, text: &str) -> pipeline::diff::DiffContent {
    pipeline::diff::DiffContent::SingleFile(pipeline::diff::SingleFile {
        file,
        lines: Arc::new(vec![text.to_owned()]),
    })
}

fn with_response(file: file_types::File, content: pipeline::diff::DiffContent) -> Harness {
    with_response_at(file, content, 60, 10)
}

fn with_response_at(
    file: file_types::File,
    content: pipeline::diff::DiffContent,
    width: u16,
    height: u16,
) -> Harness {
    let (pipeline, redraws) = fake_pipeline(Vec::new(), vec![content]);
    let mut harness =
        Harness::new::<DiffViewer>(DiffViewerProps {}, width, height).provide::<Ui>(Context {
            theme: Rc::new(Theme::DARK),
            file: Some(Rc::new(file)),
            diff_service: Some(pipeline.clone()),
            ..Context::default()
        });
    wait(&pipeline, &redraws, &mut harness);
    harness
}

#[component]
fn ViewerHost(
    scope: &mut Scope,
    file: Rc<file_types::File>,
    pipeline: Rc<Pipeline>,
    watcher_service: Rc<WatcherService>,
) -> Node {
    let _ = scope;
    rsx! {
        Ui {
            value: Context {
                theme: Rc::new(Theme::DARK),
                file: Some(Rc::clone(file)),
                diff_service: Some(Rc::clone(pipeline)),
                watcher_service: Some(Rc::clone(watcher_service)),
                ..Context::default()
            },
            DiffViewer {}
        }
    }
}

#[component]
fn ExplorerAndViewerHost(
    scope: &mut Scope,
    file: Rc<file_types::File>,
    pipeline: Rc<Pipeline>,
    watcher_service: Rc<WatcherService>,
) -> Node {
    let _ = scope;
    rsx! {
        Ui {
            value: Context {
                theme: Rc::new(Theme::DARK),
                file: Some(Rc::clone(file)),
                diff_service: Some(Rc::clone(pipeline)),
                watcher_service: Some(Rc::clone(watcher_service)),
                ..Context::default()
            },
            Row {
                layout: Layout { grow: 1, ..Default::default() },
                ..,
                Explorer {}
                DiffViewer {}
            }
        }
    }
}

#[test]
fn no_file_shows_welcome() {
    let mut harness =
        Harness::new::<DiffViewer>(DiffViewerProps {}, 60, 10).provide::<Ui>(Context {
            theme: Rc::new(Theme::DARK),
            ..Context::default()
        });
    let text = harness.screen().join("\n");

    assert!(text.contains("Select a file"), "got {text:?}");
}

#[test]
fn a_file_that_cannot_be_read_says_why() {
    let selected = file("selected.rs");
    let (pipeline, redraws) = fake_pipeline(Vec::new(), Vec::new());
    let mut harness =
        Harness::new::<DiffViewer>(DiffViewerProps {}, 60, 10).provide::<Ui>(Context {
            theme: Rc::new(Theme::DARK),
            file: Some(Rc::new(selected)),
            diff_service: Some(pipeline.clone()),
            ..Context::default()
        });
    wait(&pipeline, &redraws, &mut harness);

    assert!(
        harness.screen_line(0).contains("cannot read selected.rs"),
        "{:?}",
        harness.screen()
    );
}

#[test]
fn the_previous_file_is_hidden_while_the_next_one_loads() {
    let first = file("first.rs");
    let second = file("second.rs");
    let (release, released) = mpsc::channel::<()>();
    let (redraw, redraws) = mpsc::channel();
    let pipeline = Rc::new(Pipeline::new(
        move || {
            let _ = redraw.send(());
        },
        |_| Ok(Vec::new()),
        move |file| {
            if file.path().as_str() == "second.rs" {
                released.recv().unwrap();
            }
            let text = format!("{} body", file.path().as_str().trim_end_matches(".rs"));
            Ok(make_single_with_text(file.clone(), &text))
        },
    ));
    let watcher_service = Rc::new(WatcherService::new());
    let mut harness = Harness::new::<ViewerHost>(
        ViewerHostProps {
            file: Rc::new(first),
            pipeline: Rc::clone(&pipeline),
            watcher_service: Rc::clone(&watcher_service),
        },
        60,
        10,
    );
    wait(&pipeline, &redraws, &mut harness);
    assert!(harness.screen().join("\n").contains("first body"));

    harness.set_props::<ViewerHost>(ViewerHostProps {
        file: Rc::new(second),
        pipeline: Rc::clone(&pipeline),
        watcher_service,
    });
    harness.force_draw();
    let text = harness.screen().join("\n");
    assert!(text.contains("Loading file…"), "got {text:?}");
    assert!(!text.contains("first body"), "got {text:?}");

    release.send(()).unwrap();
    wait(&pipeline, &redraws, &mut harness);
    let text = harness.screen().join("\n");
    assert!(text.contains("second body"), "got {text:?}");
    assert!(!text.contains("first body"), "got {text:?}");
}

#[test]
fn a_worktree_change_refreshes_the_selected_file() {
    let selected = file("selected.rs");
    let (pipeline, redraws) = fake_pipeline(
        Vec::new(),
        vec![
            make_single_with_text(selected.clone(), "before refresh"),
            make_single_with_text(selected.clone(), "after refresh"),
        ],
    );
    let watcher_service = Rc::new(WatcherService::new());
    let mut harness = Harness::new::<ViewerHost>(
        ViewerHostProps {
            file: Rc::new(selected),
            pipeline: Rc::clone(&pipeline),
            watcher_service: Rc::clone(&watcher_service),
        },
        60,
        10,
    );
    harness.force_draw();
    wait(&pipeline, &redraws, &mut harness);
    harness.force_draw().force_draw();
    assert!(harness.screen().join("\n").contains("before refresh"));

    watcher_service.deliver(watcher::Refresh {
        worktree: true,
        ..watcher::Refresh::default()
    });
    wait(&pipeline, &redraws, &mut harness);
    harness.force_draw().force_draw();

    let screen = harness.screen().join("\n");
    assert!(screen.contains("after refresh"), "got {screen:?}");
    assert!(!screen.contains("before refresh"), "got {screen:?}");
}

#[test]
fn a_single_file_shows_its_content() {
    let file = file("untracked.rs");
    let mut harness = with_response(file.clone(), make_single(file));
    let text = harness.screen().join("\n");

    assert!(text.contains("untracked body"), "got {text:?}");
    assert!(!text.contains("Select a file"), "got {text:?}");
}

#[test]
fn a_diff_shows_file_content() {
    let file = file("test.rs");
    let mut harness = with_response(file.clone(), make_diff(file));
    let text = harness.screen().join("\n");

    assert!(text.contains("hello"), "got {text:?}");
    assert!(!text.contains("Select a file"), "got {text:?}");
}

#[test]
fn t_switches_a_focused_diff_layout() {
    let file = file("test.rs");
    let mut harness = with_response(file.clone(), make_diff(file));

    assert_eq!(harness.focused_name(), Some("SideBySide"));
    assert!(harness.screen().iter().any(|line| line.contains('│')));

    harness.press(crokey::key!(t)).force_draw();

    assert_eq!(harness.focused_name(), Some("Inline"));
    assert!(!harness.screen().iter().any(|line| line.contains('│')));

    harness.press(crokey::key!(t)).force_draw();

    assert_eq!(harness.focused_name(), Some("SideBySide"));
    assert!(harness.screen().iter().any(|line| line.contains('│')));
}

#[test]
fn w_toggles_wrapping_for_the_active_diff() {
    let file = file("test.rs");
    let line = "ABCDEFGHIJKLMNOPQRSTUVWXYZ0123456789";
    let content = make_diff_with_lines(file.clone(), &[line], &[line]);
    let mut harness = with_response_at(file, content, 30, 4);

    let wrapped = harness.screen();
    harness.press(crokey::key!(l)).force_draw();
    assert_eq!(harness.screen(), wrapped);

    harness.press(crokey::key!(w)).force_draw();
    let unwrapped = harness.screen();
    harness.press(crokey::key!(l)).force_draw();
    assert_ne!(harness.screen(), unwrapped);
}

#[test]
fn t_does_not_change_a_single_file() {
    let file = file("untracked.rs");
    let mut harness = with_response(file.clone(), make_single(file));
    harness.click(20, 0).force_draw();
    let before = harness.screen();

    assert_eq!(harness.focused_name(), Some("SingleFile"));
    harness.press(crokey::key!(t)).force_draw();

    assert_eq!(harness.focused_name(), Some("SingleFile"));
    assert_eq!(harness.screen(), before);
}

#[test]
fn a_diff_view_state_survives_a_layout_switch() {
    let file = file("test.rs");
    let original: Vec<String> = (1..=20)
        .map(|line| format!("line {line:02} 01234567890123456789012345678901234567890123456789"))
        .collect();
    let original: Vec<&str> = original.iter().map(String::as_str).collect();
    let content = make_diff_with_lines(file.clone(), &original, &original);
    let mut harness = with_response(file, content);

    for _ in 0..4 {
        harness.press(crokey::key!(j)).force_draw();
    }
    for _ in 0..3 {
        harness.press(crokey::key!(l)).force_draw();
    }
    let side_by_side = harness.screen();

    harness.press(crokey::key!(t)).force_draw();
    assert!(
        !harness.screen_line(0).contains("line 01"),
        "layout switch reset the terminal position: {:?}",
        harness.screen_line(0)
    );
    assert!(!harness.screen().iter().any(|line| line.contains('│')));

    harness.press(crokey::key!(t)).force_draw();

    assert_eq!(harness.screen(), side_by_side);
}

#[test]
fn the_first_screen_view_line_is_preserved_without_an_offset() {
    let file = file("test.rs");
    let original = ["shared 01", "old 02", "old 03", "shared 04"];
    let modified = ["shared 01", "new 02", "shared 04"];
    let mut harness = with_response_at(
        file.clone(),
        make_diff_with_lines(file, &original, &modified),
        60,
        2,
    );

    harness
        .press(crokey::key!(j))
        .press(crokey::key!(j))
        .press(crokey::key!(j))
        .force_draw();
    let side_by_side = harness.screen();
    assert!(side_by_side[0].contains("old 03"), "got {side_by_side:?}");

    harness.press(crokey::key!(t)).force_draw();

    assert!(harness.screen_line(0).contains("old 03"));
    assert!(!harness.screen().iter().any(|line| line.contains('│')));
}

#[test]
fn a_diff_view_state_survives_switching_files() {
    let first = file("first.rs");
    let second = file("second.rs");
    let first_lines: Vec<String> = (1..=20).map(|line| format!("first {line:02}")).collect();
    let second_lines: Vec<String> = (1..=20).map(|line| format!("second {line:02}")).collect();
    let first_lines: Vec<&str> = first_lines.iter().map(String::as_str).collect();
    let second_lines: Vec<&str> = second_lines.iter().map(String::as_str).collect();
    let (pipeline, redraws) = fake_pipeline(
        Vec::new(),
        vec![
            make_diff_with_lines(first.clone(), &first_lines, &first_lines),
            make_diff_with_lines(second.clone(), &second_lines, &second_lines),
            make_diff_with_lines(first.clone(), &first_lines, &first_lines),
        ],
    );
    let watcher_service = Rc::new(WatcherService::new());
    let mut harness = Harness::new::<ViewerHost>(
        ViewerHostProps {
            file: Rc::new(first),
            pipeline: Rc::clone(&pipeline),
            watcher_service: Rc::clone(&watcher_service),
        },
        60,
        10,
    );
    harness.force_draw();
    wait(&pipeline, &redraws, &mut harness);
    harness.force_draw().force_draw();
    for _ in 0..3 {
        harness.press(crokey::key!(j)).force_draw();
    }
    assert!(harness.screen_line(0).contains("first 04"));

    harness.set_props::<ViewerHost>(ViewerHostProps {
        file: Rc::new(second),
        pipeline: Rc::clone(&pipeline),
        watcher_service: Rc::clone(&watcher_service),
    });
    harness.force_draw();
    wait(&pipeline, &redraws, &mut harness);
    harness.force_draw().force_draw();
    assert!(harness.screen_line(0).contains("second 01"));

    harness.set_props::<ViewerHost>(ViewerHostProps {
        file: Rc::new(file("first.rs")),
        pipeline: Rc::clone(&pipeline),
        watcher_service: Rc::clone(&watcher_service),
    });
    harness.force_draw();
    wait(&pipeline, &redraws, &mut harness);
    harness.force_draw().force_draw();
    assert!(harness.screen_line(0).contains("first 04"));
}

#[test]
fn t_does_not_bubble_from_the_explorer_to_the_diff_viewer() {
    let file = file("test.rs");
    let content = make_diff(file.clone());
    let (pipeline, redraws) = fake_pipeline(Vec::new(), vec![content]);
    let watcher_service = Rc::new(WatcherService::new());
    let mut harness = Harness::new::<ExplorerAndViewerHost>(
        ExplorerAndViewerHostProps {
            file: Rc::new(file),
            pipeline: Rc::clone(&pipeline),
            watcher_service,
        },
        100,
        10,
    )
    .provide::<Ui>(Context {
        theme: Rc::new(Theme::DARK),
        ..Context::default()
    });
    harness.force_draw();
    wait(&pipeline, &redraws, &mut harness);
    harness.force_draw().force_draw();

    assert_eq!(harness.focused_name(), Some("Explorer"));
    let before = harness.screen();
    harness.press(crokey::key!(t)).force_draw();

    assert_eq!(harness.focused_name(), Some("Explorer"));
    assert_eq!(harness.screen(), before);
}
