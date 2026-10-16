use std::cell::RefCell;
use std::path::Path;
use std::rc::Rc;
use std::sync::mpsc;
use std::time::Duration;

use file_types::{File, Oid, RepoPath, Revs, Stats};
use loom::testing::Harness;
use loom::{Basis, Layout, Row, RowProps, Scope, component, rsx, use_state};
use ui::Theme;
use ui::components::border::{Border, BorderProps};
use ui::components::diff_viewer::DiffViewer;
use ui::components::explorer::Explorer;
use ui::components::{Context, DirectorySelection, Ui, UiProps};
use ui::services::diff::DiffService;
use ui::services::files::FilesService;

fn file(path: &str) -> File {
    File::unchanged_path(
        RepoPath::new(path, Path::new("/repo")),
        Revs::worktree_against(Oid::new("base")),
    )
    .set_stats(Stats::new(1, 1))
}

fn diff(file: File, original: &str, modified: &str) -> pipeline::diff::DiffContent {
    let original: Vec<_> = original.lines().collect();
    let modified: Vec<_> = modified.lines().collect();
    let changed =
        pipeline::diff::compute(&original, &modified, pipeline::diff::Settings::default())
            .expect("diff computes");
    let alignment = pipeline::diff::align(changed, &original, &modified).expect("alignment");
    pipeline::diff::DiffContent::Diff(pipeline::diff::Diff { file, alignment })
}

#[component]
fn SelectedDiffHost(_scope: &mut Scope, context: Context) -> loom::Node {
    rsx! {
        Ui {
            value: context.clone(),
            DiffViewer {}
        }
    }
}

#[component]
fn ExplorerDiffHost(
    scope: &mut Scope,
    files_service: Rc<FilesService>,
    diff_service: Rc<DiffService>,
) -> loom::Node {
    let (file, set_file) = use_state(scope, || None::<Rc<File>>);
    let (directory, set_directory) = use_state(scope, || None::<Rc<DirectorySelection>>);
    rsx! {
        Ui {
            value: Context {
                theme: Rc::new(Theme::DARK),
                repo: Rc::from(Path::new("/repo")),
                file,
                set_file: Some(set_file),
                directory,
                set_directory: Some(set_directory),
                files_service: Some(Rc::clone(files_service)),
                diff_service: Some(Rc::clone(diff_service)),
                ..Context::default()
            },
            Row {
                layout: Layout { grow: 1, ..Default::default() },
                ..,
                Border {
                    layout: Layout { basis: Basis::Length(32), shrink: 1, ..Default::default() },
                    Explorer {}
                }
                Border {
                    layout: Layout { grow: 1, ..Default::default() },
                    DiffViewer {}
                }
            }
        }
    }
}

fn setup(
    line_count: usize,
) -> (
    Harness,
    Rc<FilesService>,
    Rc<DiffService>,
    mpsc::Receiver<pipeline::diff::Response>,
) {
    let first = file("src/a.rs");
    let second = file("src/b.rs");
    let added = File::added(
        RepoPath::new("src/new.rs", Path::new("/repo")),
        Revs::worktree_against(Oid::new("base")),
    );
    let outside = file("z-readme.rs");
    let files = vec![first.clone(), second.clone(), added, outside];

    let (files_tx, files_responses) = mpsc::channel();
    let files_worker = pipeline::files::FilesWorker::mock(
        vec![files],
        channel::Emitter::new(files_tx, |response| response),
    );
    let files_service = Rc::new(FilesService::new(
        Rc::new(RefCell::new(files_worker)),
        Vec::new(),
    ));

    let text = |marker: &str| {
        (0..line_count)
            .map(|line| format!("{marker}_{line:02}"))
            .collect::<Vec<_>>()
            .join("\n")
    };
    let (diff_tx, diff_responses) = mpsc::channel();
    let diff_worker = pipeline::diff::DiffWorker::mock_by_file(
        vec![
            diff(first.clone(), &text("old_a_marker"), &text("new_a_marker")),
            diff(second.clone(), &text("old_b_marker"), &text("new_b_marker")),
        ],
        channel::Emitter::new(diff_tx, |response| response),
    );
    let diff_service = Rc::new(DiffService::new(Rc::new(RefCell::new(diff_worker))));

    let mut harness = Harness::new::<ExplorerDiffHost>(
        ExplorerDiffHostProps {
            files_service: Rc::clone(&files_service),
            diff_service: Rc::clone(&diff_service),
        },
        100,
        18,
    );
    harness.force_draw();
    files_service.deliver(
        files_responses
            .recv_timeout(Duration::from_secs(1))
            .expect("files response"),
    );
    harness.force_draw();
    (harness, files_service, diff_service, diff_responses)
}

#[test]
fn folding_a_selected_directory_keeps_its_loaded_cards_and_scroll_position() {
    let (mut harness, _files_service, service, responses) = setup(32);
    let directory_row = harness
        .screen()
        .iter()
        .position(|line| line.chars().take(32).collect::<String>().contains("src"))
        .expect("src directory");
    harness.click(2, directory_row as u16).force_draw();
    for _ in 0..2 {
        service.deliver(
            responses
                .recv_timeout(Duration::from_secs(1))
                .expect("directory diff"),
        );
        harness.force_draw();
    }
    let initial = harness.screen();
    harness.wheel(80, 6, 1).force_draw();
    assert_ne!(
        harness.screen(),
        initial,
        "the cards must be scrolled before folding"
    );
    let expected = harness.cells().clone();
    let area = harness.area_of("DiffViewer").expect("diff viewer area");
    for mouse in [true, false] {
        for expanded in [false, true] {
            if mouse {
                harness.click(2, directory_row as u16).force_draw();
            } else {
                harness.press(loom::crokey::key!(enter)).force_draw();
            }
            let screen = harness.screen();
            let explorer = screen
                .iter()
                .map(|line| line.chars().take(32).collect::<String>())
                .collect::<Vec<_>>()
                .join("\n");
            assert_eq!(
                explorer.contains("a.rs"),
                expanded,
                "directory did not toggle"
            );
            assert!(
                !screen.join("\n").contains("Loading directory diffs…"),
                "folding reloaded the cards"
            );
            let actual = harness.cells().clone();
            for y in area.y..area.bottom() {
                for x in area.x..area.right() {
                    assert_eq!(
                        actual.cell((x, y)),
                        expected.cell((x, y)),
                        "card cell changed at {x},{y} after folding"
                    );
                }
            }
        }
    }
    assert!(
        matches!(
            responses.recv_timeout(Duration::from_millis(100)),
            Err(mpsc::RecvTimeoutError::Timeout)
        ),
        "folding issued another diff request"
    );
}

#[test]
fn changing_directories_clears_old_cards_and_resets_scrolling() {
    let first = file("first/a.rs");
    let second = file("second/b.rs");
    let (tx, responses) = mpsc::channel();
    let content = |file: &File, marker: &str| {
        let original = (0..32)
            .map(|line| format!("old_{marker}_{line:02}"))
            .collect::<Vec<_>>()
            .join("\n");
        let modified = (0..32)
            .map(|line| format!("new_{marker}_{line:02}"))
            .collect::<Vec<_>>()
            .join("\n");
        diff(file.clone(), &original, &modified)
    };
    let worker = pipeline::diff::DiffWorker::mock_by_file(
        vec![content(&first, "a"), content(&second, "b")],
        channel::Emitter::new(tx, |response| response),
    );
    let service = Rc::new(DiffService::new(Rc::new(RefCell::new(worker))));
    let directory = |path: &str, file: &File| {
        Rc::new(DirectorySelection {
            path: RepoPath::new(path, Path::new("/repo")),
            revs: file.revs().clone(),
            files: Rc::new(vec![file.clone()]),
        })
    };
    let first_context = Context {
        directory: Some(directory("first", &first)),
        diff_service: Some(Rc::clone(&service)),
        ..Context::default()
    };
    let second_context = Context {
        directory: Some(directory("second", &second)),
        ..first_context.clone()
    };
    let mut harness = Harness::new::<SelectedDiffHost>(
        SelectedDiffHostProps {
            context: first_context.clone(),
        },
        80,
        10,
    );
    harness.force_draw();
    service.deliver(
        responses
            .recv_timeout(Duration::from_secs(1))
            .expect("first directory"),
    );
    harness.force_draw();
    assert!(harness.screen().join("\n").contains("first/a.rs"));
    harness.wheel(50, 5, 1).force_draw();
    assert!(!harness.screen().join("\n").contains("first/a.rs"));

    let card_renders = harness.render_count_of("DiffCards");
    harness
        .set_props::<SelectedDiffHost>(SelectedDiffHostProps {
            context: second_context.clone(),
        })
        .force_draw();
    assert_eq!(
        harness.render_count_of("DiffCards"),
        card_renders,
        "old cards rendered while the new directory was loading"
    );
    assert!(
        harness
            .screen()
            .join("\n")
            .contains("Loading directory diffs…")
    );
    service.deliver(pipeline::diff::Response {
        file: first.clone(),
        content: Ok(content(&first, "a")),
    });
    harness.force_draw();
    assert!(
        harness
            .screen()
            .join("\n")
            .contains("Loading directory diffs…")
    );
    service.deliver(
        responses
            .recv_timeout(Duration::from_secs(1))
            .expect("second directory"),
    );
    harness.force_draw();
    let second_screen = harness.screen();
    let text = second_screen.join("\n");
    assert!(
        text.contains("second/b.rs") && text.contains("new_b_00"),
        "{text}"
    );
    assert!(!text.contains("new_a_"), "{text}");

    harness
        .set_props::<SelectedDiffHost>(SelectedDiffHostProps {
            context: Context {
                directory: None,
                file: Some(Rc::new(first.clone())),
                ..first_context
            },
        })
        .force_draw();
    service.deliver(
        responses
            .recv_timeout(Duration::from_secs(1))
            .expect("selected file"),
    );
    harness.force_draw();
    assert!(harness.screen().join("\n").contains("new_a_00"));
    harness
        .set_props::<SelectedDiffHost>(SelectedDiffHostProps {
            context: second_context,
        })
        .force_draw();
    assert!(
        harness
            .screen()
            .join("\n")
            .contains("Loading directory diffs…")
    );
    service.deliver(
        responses
            .recv_timeout(Duration::from_secs(1))
            .expect("reselected directory"),
    );
    harness.force_draw();
    assert_eq!(harness.screen(), second_screen);
}

#[test]
fn selecting_a_directory_shows_its_two_sided_diffs_and_a_file_keeps_the_regular_viewer() {
    let (mut harness, _files_service, diff_service, responses) = setup(1);

    harness.press(loom::crokey::key!(j)).force_draw();
    for _ in 0..2 {
        diff_service.deliver(
            responses
                .recv_timeout(Duration::from_secs(1))
                .expect("directory diff response"),
        );
        harness.force_draw();
    }
    let cards = harness
        .screen()
        .iter()
        .map(|line| line.chars().skip(35).collect::<String>())
        .collect::<Vec<_>>()
        .join("\n");
    assert!(cards.contains("src/a.rs"), "{cards}");
    assert!(cards.contains("src/b.rs"), "{cards}");
    assert!(cards.contains("new_a_marker"), "{cards}");
    assert!(
        !cards.contains("src/new.rs"),
        "one-sided files are not cards: {cards}"
    );
    assert!(
        !cards.contains("z-readme.rs"),
        "files outside the directory are omitted: {cards}"
    );

    let selected_file_row = harness
        .screen()
        .iter()
        .position(|line| line.chars().take(32).collect::<String>().contains("a.rs"))
        .expect("Explorer lists src/a.rs");
    harness.click(2, selected_file_row as u16).force_draw();
    diff_service.deliver(
        responses
            .recv_timeout(Duration::from_secs(1))
            .expect("selected file diff response"),
    );
    harness.force_draw();

    let file_view = harness.screen().join("\n");
    assert!(file_view.contains("new_a_marker"), "{file_view}");
    assert!(
        !file_view.contains("new_b_marker"),
        "directory cards remain visible: {file_view}"
    );
}
