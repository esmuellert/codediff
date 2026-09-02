use std::cell::RefCell;
use std::path::Path;
use std::rc::Rc;
use std::sync::Arc;
use std::sync::mpsc;
use std::time::Duration;

use loom::testing::Harness;
use loom::{Node, Scope, component, rsx};
use ui::Theme;
use ui::components::diff_viewer::{DiffViewer, DiffViewerProps};
use ui::components::{Context, Ui, UiProps};
use ui::services::diff::DiffService;

fn file(path: &str) -> file_types::File {
    file_types::File::unchanged_path(
        file_types::RepoPath::new(path, Path::new("/repo")),
        file_types::Revs::worktree_against(file_types::Oid::new("abc")),
    )
}

fn make_diff(file: file_types::File) -> pipeline::diff::DiffContent {
    let original = ["hello", "world"];
    let modified = ["hello", "world"];
    let diff = pipeline::diff::compute(&original, &modified).expect("a diff");
    let alignment = pipeline::diff::align(diff, &original, &modified).expect("alignment");
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

fn pending_response(
    file: file_types::File,
    content: pipeline::diff::DiffContent,
) -> (
    Harness,
    Rc<DiffService>,
    mpsc::Receiver<pipeline::diff::Response>,
) {
    let (diff_tx, diff_responses) = mpsc::channel();
    let diff_worker = pipeline::diff::DiffWorker::mock(
        vec![Ok(content)],
        channel::Emitter::new(diff_tx, |response| response),
    );
    let diff_service = Rc::new(DiffService::new(Rc::new(RefCell::new(diff_worker))));
    let mut harness =
        Harness::new::<DiffViewer>(DiffViewerProps {}, 60, 10).provide::<Ui>(Context {
            theme: Rc::new(Theme::DARK),
            file: Some(Rc::new(file)),
            diff_service: Some(Rc::clone(&diff_service)),
            ..Context::default()
        });
    harness.force_draw();
    (harness, diff_service, diff_responses)
}

fn with_response(file: file_types::File, content: pipeline::diff::DiffContent) -> Harness {
    let (mut harness, diff_service, diff_responses) = pending_response(file, content);
    let response = diff_responses
        .recv_timeout(Duration::from_secs(1))
        .expect("diff response");
    diff_service.deliver(response);
    harness.force_draw().force_draw();
    harness
}

#[component]
fn ViewerHost(
    scope: &mut Scope,
    file: Rc<file_types::File>,
    diff_service: Rc<DiffService>,
) -> Node {
    let _ = scope;
    rsx! {
        Ui {
            value: Context {
                theme: Rc::new(Theme::DARK),
                file: Some(Rc::clone(file)),
                diff_service: Some(Rc::clone(diff_service)),
                ..Context::default()
            },
            DiffViewer {}
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
fn a_response_for_another_file_is_ignored() {
    let selected = file("selected.rs");
    let content = make_single(selected.clone());
    let (mut harness, diff_service, diff_responses) = pending_response(selected, content);
    let mut response = diff_responses
        .recv_timeout(Duration::from_secs(1))
        .expect("diff response");
    response.file = file("other.rs");
    diff_service.deliver(response);
    harness.force_draw().force_draw();

    let text = harness.screen().join("\n");
    assert!(text.contains("Select a file"), "got {text:?}");
    assert!(!text.contains("untracked body"), "got {text:?}");
}

#[test]
fn the_previous_file_stays_visible_until_the_next_response() {
    let first = file("first.rs");
    let second = file("second.rs");
    let (diff_tx, diff_responses) = mpsc::channel();
    let diff_worker = pipeline::diff::DiffWorker::mock(
        vec![
            Ok(make_single_with_text(first.clone(), "first body")),
            Ok(make_single_with_text(second.clone(), "second body")),
        ],
        channel::Emitter::new(diff_tx, |response| response),
    );
    let diff_service = Rc::new(DiffService::new(Rc::new(RefCell::new(diff_worker))));
    let mut harness = Harness::new::<ViewerHost>(
        ViewerHostProps {
            file: Rc::new(first),
            diff_service: Rc::clone(&diff_service),
        },
        60,
        10,
    );
    harness.force_draw();
    diff_service.deliver(
        diff_responses
            .recv_timeout(Duration::from_secs(1))
            .expect("first diff response"),
    );
    harness.force_draw().force_draw();
    assert!(harness.screen().join("\n").contains("first body"));

    harness.set_props::<ViewerHost>(ViewerHostProps {
        file: Rc::new(second),
        diff_service: Rc::clone(&diff_service),
    });
    harness.force_draw();
    assert!(harness.screen().join("\n").contains("first body"));

    diff_service.deliver(
        diff_responses
            .recv_timeout(Duration::from_secs(1))
            .expect("second diff response"),
    );
    harness.force_draw().force_draw();
    let text = harness.screen().join("\n");
    assert!(text.contains("second body"), "got {text:?}");
    assert!(!text.contains("first body"), "got {text:?}");
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
