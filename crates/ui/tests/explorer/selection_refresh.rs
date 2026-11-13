use std::cell::RefCell;
use std::path::Path;
use std::rc::Rc;
use std::sync::Arc;
use std::sync::mpsc::Receiver;

use diff_service::DiffService;
use loom::testing::Harness;
use loom::{Layout, Node, Row, RowProps, Scope, component, rsx};
use ui::Theme;
use ui::components::diff_viewer::DiffViewer;
use ui::components::{Context, Explorer, Ui, UiProps};
use ui::services::watcher::WatcherService;

use super::common::{fake_pipeline, file, file_with_stats, wait};

#[component]
fn SelectionHost(
    scope: &mut Scope,
    pipeline: Rc<DiffService>,
    watcher_service: Rc<WatcherService>,
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
                diff_service: Some(Rc::clone(pipeline)),
                watcher_service: Some(Rc::clone(watcher_service)),
                ..Context::default()
            },
            Explorer {}
        }
    }
}

#[component]
fn ReviewHost(
    scope: &mut Scope,
    pipeline: Rc<DiffService>,
    watcher_service: Rc<WatcherService>,
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
                diff_service: Some(Rc::clone(pipeline)),
                watcher_service: Some(Rc::clone(watcher_service)),
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
    pipeline: Rc<DiffService>,
    watcher_service: Rc<WatcherService>,
    redraws: Receiver<()>,
    observed_file: Rc<RefCell<Option<file_types::File>>>,
}

fn selection_harness(
    lists: Vec<Vec<file_types::File>>,
    selected: file_types::File,
) -> SelectionHarness {
    let (pipeline, redraws) = fake_pipeline(lists, Vec::new());
    let watcher_service = Rc::new(WatcherService::new());
    let observed_file = Rc::new(RefCell::new(None));
    let mut harness = Harness::new::<SelectionHost>(
        SelectionHostProps {
            pipeline: Rc::clone(&pipeline),
            watcher_service: Rc::clone(&watcher_service),
            initial_file: Rc::new(selected),
            observed_file: Rc::clone(&observed_file),
        },
        40,
        5,
    );
    wait(&pipeline, &redraws, &mut harness);
    SelectionHarness {
        harness,
        pipeline,
        watcher_service,
        redraws,
        observed_file,
    }
}

fn refresh(harness: &mut SelectionHarness) {
    harness.watcher_service.deliver(watcher::Refresh {
        worktree: true,
        ..watcher::Refresh::default()
    });
    wait(&harness.pipeline, &harness.redraws, &mut harness.harness);
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
    let mut harness = selection_harness(vec![vec![selected.clone()], Vec::new()], selected);

    refresh(&mut harness);

    assert!(harness.observed_file.borrow().is_none());
}

#[test]
fn clearing_the_last_change_replaces_its_diff_with_welcome() {
    let selected = added_file("selected.rs");
    let content = diff_service::diff::DiffContent::SingleFile(diff_service::diff::SingleFile {
        file: selected.clone(),
        lines: Arc::new(vec!["stale body".to_owned()]),
    });
    let (pipeline, redraws) =
        fake_pipeline(vec![vec![selected.clone()], Vec::new()], vec![content]);
    let watcher_service = Rc::new(WatcherService::new());
    let mut harness = Harness::new::<ReviewHost>(
        ReviewHostProps {
            pipeline: Rc::clone(&pipeline),
            watcher_service: Rc::clone(&watcher_service),
            initial_file: Rc::new(selected),
        },
        100,
        12,
    );
    wait(&pipeline, &redraws, &mut harness);
    assert!(harness.screen().join("\n").contains("stale body"));

    watcher_service.deliver(watcher::Refresh {
        worktree: true,
        ..watcher::Refresh::default()
    });
    wait(&pipeline, &redraws, &mut harness);

    let screen = harness.screen().join("\n");
    assert!(screen.contains("Select a file"), "got {screen:?}");
    assert!(!screen.contains("stale body"), "got {screen:?}");
}

#[test]
fn removing_the_selected_file_clears_it_while_other_files_remain() {
    let selected = file("selected.rs");
    let other = file("other.rs");
    let mut harness = selection_harness(
        vec![vec![selected.clone(), other.clone()], vec![other]],
        selected,
    );

    refresh(&mut harness);

    assert!(harness.observed_file.borrow().is_none());
}

#[test]
fn a_surviving_selection_uses_the_refreshed_file() {
    let selected = file("selected.rs");
    let refreshed = file_with_stats("selected.rs", 4, 2);
    let mut harness = selection_harness(
        vec![vec![selected.clone()], vec![refreshed.clone()]],
        selected,
    );

    refresh(&mut harness);

    assert_eq!(harness.observed_file.borrow().as_ref(), Some(&refreshed));
}

#[test]
fn the_same_path_in_another_comparison_does_not_keep_the_selection() {
    let selected = staged_file("selected.rs");
    let mut harness = selection_harness(
        vec![vec![selected.clone()], vec![file("selected.rs")]],
        selected,
    );

    refresh(&mut harness);

    assert!(harness.observed_file.borrow().is_none());
}
