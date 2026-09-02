use std::cell::RefCell;
use std::path::Path;
use std::rc::Rc;
use std::sync::Arc;
use std::sync::mpsc;
use std::time::Duration;

use loom::testing::Harness;
use loom::{Layout, Node, Row, RowProps, Scope, component, rsx};
use ui::Theme;
use ui::components::diff_viewer::DiffViewer;
use ui::components::{Context, Explorer, Ui, UiProps};
use ui::services::diff::DiffService;
use ui::services::files::FilesService;

use super::common::{file, file_with_stats, mock_file_service};

#[component]
fn SelectionHost(
    scope: &mut Scope,
    file_service: Rc<FilesService>,
    initial_file: Rc<file_types::File>,
    observed_file: Rc<RefCell<Option<file_types::File>>>,
) -> Node {
    let initial_file = Rc::clone(initial_file);
    let (file, set_file) = loom::use_state(scope, || Some(initial_file));
    *observed_file.borrow_mut() = file.as_deref().cloned();
    rsx! {
        Ui {
            value: Context {
                theme: Rc::new(Theme::DARK),
                repo: Rc::from(Path::new("/repo")),
                file: file.as_ref().map(Rc::clone),
                set_file: Some(set_file),
                file_service: Some(Rc::clone(file_service)),
                ..Context::default()
            },
            Explorer {}
        }
    }
}

#[component]
fn ReviewHost(
    scope: &mut Scope,
    file_service: Rc<FilesService>,
    diff_service: Rc<DiffService>,
    initial_file: Rc<file_types::File>,
) -> Node {
    let initial_file = Rc::clone(initial_file);
    let (file, set_file) = loom::use_state(scope, || Some(initial_file));
    rsx! {
        Ui {
            value: Context {
                theme: Rc::new(Theme::DARK),
                repo: Rc::from(Path::new("/repo")),
                file: file.as_ref().map(Rc::clone),
                set_file: Some(set_file),
                file_service: Some(Rc::clone(file_service)),
                diff_service: Some(Rc::clone(diff_service)),
                ..Context::default()
            },
            Row {
                layout: Layout { grow: 1, ..Layout::default() },
                ..,
                Explorer {}
                DiffViewer {}
            }
        }
    }
}

struct SelectionHarness {
    harness: Harness,
    file_service: Rc<FilesService>,
    responses: std::sync::mpsc::Receiver<pipeline::files::Response>,
    observed_file: Rc<RefCell<Option<file_types::File>>>,
}

fn selection_harness(
    responses: Vec<Vec<file_types::File>>,
    selected: file_types::File,
) -> SelectionHarness {
    let (file_service, responses) = mock_file_service(responses);
    let observed_file = Rc::new(RefCell::new(None));
    let mut harness = Harness::new::<SelectionHost>(
        SelectionHostProps {
            file_service: Rc::clone(&file_service),
            initial_file: Rc::new(selected),
            observed_file: Rc::clone(&observed_file),
        },
        40,
        5,
    );
    harness.force_draw();
    receive(&mut harness, &file_service, &responses);
    SelectionHarness {
        harness,
        file_service,
        responses,
        observed_file,
    }
}

fn receive(
    harness: &mut Harness,
    file_service: &FilesService,
    responses: &std::sync::mpsc::Receiver<pipeline::files::Response>,
) {
    file_service.deliver(
        responses
            .recv_timeout(Duration::from_secs(1))
            .expect("file list response"),
    );
    harness.force_draw().force_draw();
}

fn refresh(
    harness: &mut Harness,
    file_service: &FilesService,
    responses: &std::sync::mpsc::Receiver<pipeline::files::Response>,
) {
    file_service.fs_changed(watcher::Refresh {
        worktree: true,
        ..watcher::Refresh::default()
    });
    receive(harness, file_service, responses);
}

fn added_file(path: &str) -> file_types::File {
    file_types::File::added(
        file_types::RepoPath::new(path, Path::new("/repo")),
        file_types::Revs::worktree_against(file_types::Oid::new("abc")),
    )
}

fn staged_file(path: &str) -> file_types::File {
    file_types::File::unchanged_path(
        file_types::RepoPath::new(path, Path::new("/repo")),
        file_types::Revs::new(
            file_types::Rev::Commit(file_types::Oid::new("abc")),
            file_types::Rev::Index,
        ),
    )
}

#[test]
fn an_empty_refresh_clears_the_selected_file() {
    let selected = file("selected.rs");
    let SelectionHarness {
        mut harness,
        file_service,
        responses,
        observed_file,
    } = selection_harness(vec![vec![selected.clone()], Vec::new()], selected);

    refresh(&mut harness, &file_service, &responses);

    assert!(observed_file.borrow().is_none());
}

#[test]
fn clearing_the_last_change_replaces_its_diff_with_welcome() {
    let selected = added_file("selected.rs");
    let (file_service, file_responses) =
        mock_file_service(vec![vec![selected.clone()], Vec::new()]);
    let (diff_tx, diff_responses) = mpsc::channel();
    let content = pipeline::diff::DiffContent::SingleFile(pipeline::diff::SingleFile {
        file: selected.clone(),
        lines: Arc::new(vec!["stale body".to_owned()]),
    });
    let diff_worker = pipeline::diff::DiffWorker::mock(
        vec![Ok(content)],
        channel::Emitter::new(diff_tx, |response| response),
    );
    let diff_service = Rc::new(DiffService::new(Rc::new(RefCell::new(diff_worker))));
    let mut harness = Harness::new::<ReviewHost>(
        ReviewHostProps {
            file_service: Rc::clone(&file_service),
            diff_service: Rc::clone(&diff_service),
            initial_file: Rc::new(selected),
        },
        100,
        12,
    );
    harness.force_draw();
    receive(&mut harness, &file_service, &file_responses);
    diff_service.deliver(
        diff_responses
            .recv_timeout(Duration::from_secs(1))
            .expect("diff response"),
    );
    harness.force_draw().force_draw();
    assert!(harness.screen().join("\n").contains("stale body"));

    refresh(&mut harness, &file_service, &file_responses);

    let screen = harness.screen().join("\n");
    assert!(screen.contains("Select a file"), "got {screen:?}");
    assert!(!screen.contains("stale body"), "got {screen:?}");
}

#[test]
fn removing_the_selected_file_clears_it_while_other_files_remain() {
    let selected = file("selected.rs");
    let other = file("other.rs");
    let SelectionHarness {
        mut harness,
        file_service,
        responses,
        observed_file,
    } = selection_harness(
        vec![vec![selected.clone(), other.clone()], vec![other]],
        selected,
    );

    refresh(&mut harness, &file_service, &responses);

    assert!(observed_file.borrow().is_none());
}

#[test]
fn a_surviving_selection_uses_the_refreshed_file() {
    let selected = file("selected.rs");
    let refreshed = file_with_stats("selected.rs", 4, 2);
    let SelectionHarness {
        mut harness,
        file_service,
        responses,
        observed_file,
    } = selection_harness(
        vec![vec![selected.clone()], vec![refreshed.clone()]],
        selected,
    );

    refresh(&mut harness, &file_service, &responses);

    assert_eq!(observed_file.borrow().as_ref(), Some(&refreshed));
}

#[test]
fn the_same_path_in_another_comparison_does_not_keep_the_selection() {
    let selected = staged_file("selected.rs");
    let SelectionHarness {
        mut harness,
        file_service,
        responses,
        observed_file,
    } = selection_harness(
        vec![vec![selected.clone()], vec![file("selected.rs")]],
        selected,
    );

    refresh(&mut harness, &file_service, &responses);

    assert!(observed_file.borrow().is_none());
}
