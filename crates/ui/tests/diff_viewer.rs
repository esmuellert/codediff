//! Tests for DiffViewer.

use std::path::Path;
use std::rc::Rc;
use std::sync::Arc;

use loom::testing::Harness;
use ui::Theme;
use ui::components::diff_viewer::{DiffViewer, DiffViewerProps};
use ui::components::{Context, Ui};

fn with_diff(diff: Option<Rc<pipeline::diff::DiffContent>>) -> Harness {
    Harness::new::<DiffViewer>(DiffViewerProps {}, 60, 10).provide::<Ui>(Context {
        theme: Rc::new(Theme::DARK),
        diff,
        ..Context::default()
    })
}

fn make_diff(original: &[&str], modified: &[&str]) -> pipeline::diff::DiffContent {
    let diff = pipeline::diff::compute(original, modified).expect("a diff");
    let alignment = pipeline::diff::align(diff, original, modified).expect("alignment");
    let file = file_types::File::unchanged_path(
        file_types::RepoPath::new("test.rs", Path::new("/repo")),
        file_types::Revs::worktree_against(file_types::Oid::new("abc")),
    );
    pipeline::diff::DiffContent::Diff(pipeline::diff::Diff { file, alignment })
}

fn make_single() -> pipeline::diff::DiffContent {
    let file = file_types::File::added(
        file_types::RepoPath::new("untracked.rs", Path::new("/repo")),
        file_types::Revs::worktree_against(file_types::Oid::new("abc")),
    );
    pipeline::diff::DiffContent::SingleFile(pipeline::diff::SingleFile {
        file,
        lines: Arc::new(vec!["untracked body".to_owned()]),
    })
}

#[test]
fn no_diff_shows_welcome() {
    let mut h = with_diff(None);
    let screen = h.screen();
    let text = screen.join("\n");
    assert!(
        text.contains("Select a file"),
        "welcome should appear when there is no diff: {screen:?}"
    );
}

#[test]
fn a_single_file_shows_its_content() {
    let mut h = with_diff(Some(Rc::new(make_single())));
    let text = h.screen().join("\n");

    assert!(text.contains("untracked body"), "got {text:?}");
    assert!(!text.contains("Select a file"), "got {text:?}");
}

#[test]
fn a_diff_shows_file_content() {
    let content = make_diff(&["hello", "world"], &["hello", "world"]);
    let mut h = with_diff(Some(Rc::new(content)));
    let screen = h.screen();
    let text = screen.join("\n");
    assert!(
        text.contains("hello"),
        "the file text should appear: {screen:?}"
    );
    assert!(
        !text.contains("Select a file"),
        "welcome should not appear when a diff is showing: {screen:?}"
    );
}
