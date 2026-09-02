use std::cell::RefCell;
use std::path::Path;
use std::rc::Rc;
use std::sync::Arc;
use std::sync::mpsc;
use std::time::Duration;

use loom::testing::Harness;
use ui::Theme;
use ui::components::diff_viewer::{DiffViewer, DiffViewerProps};
use ui::components::{Context, Ui};
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
    pipeline::diff::DiffContent::SingleFile(pipeline::diff::SingleFile {
        file,
        lines: Arc::new(vec!["untracked body".to_owned()]),
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
    let (tx, rx) = mpsc::channel();
    let worker = pipeline::diff::DiffWorker::mock(
        vec![Ok(content)],
        channel::Emitter::new(tx, |response| response),
    );
    let service = Rc::new(DiffService::new(Rc::new(RefCell::new(worker))));
    let mut harness =
        Harness::new::<DiffViewer>(DiffViewerProps {}, 60, 10).provide::<Ui>(Context {
            theme: Rc::new(Theme::DARK),
            file: Some(Rc::new(file)),
            diff_service: Some(Rc::clone(&service)),
            ..Context::default()
        });
    harness.force_draw();
    (harness, service, rx)
}

fn with_response(file: file_types::File, content: pipeline::diff::DiffContent) -> Harness {
    let (mut harness, service, rx) = pending_response(file, content);
    let response = rx
        .recv_timeout(Duration::from_secs(1))
        .expect("diff response");
    service.deliver(response);
    harness.force_draw().force_draw();
    harness
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
    let (mut harness, service, rx) = pending_response(selected, content);
    let mut response = rx
        .recv_timeout(Duration::from_secs(1))
        .expect("diff response");
    response.file = file("other.rs");
    service.deliver(response);
    harness.force_draw().force_draw();

    let text = harness.screen().join("\n");
    assert!(text.contains("Select a file"), "got {text:?}");
    assert!(!text.contains("untracked body"), "got {text:?}");
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
