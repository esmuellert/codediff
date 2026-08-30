//! Tests for DiffViewer.

use std::path::Path;
use std::rc::Rc;

use loom::testing::Harness;
use ui::Theme;
use ui::components::diff_viewer::{DiffViewer, DiffViewerProps};
use ui::components::{Context, Ui};

fn with_diff(diff: Option<Rc<pipeline::diff::DiffContent>>) -> Harness {
    let rows = 10u32;
    Harness::new::<DiffViewer>(DiffViewerProps {}, 60, rows as u16)
        .provide::<Ui>(Context {
            theme: Rc::new(Theme::DARK),
            view_lines: 0..rows,
            diff,
            ..Context::default()
        })
}

fn make_diff(original: &[&str], modified: &[&str]) -> pipeline::diff::DiffContent {
    let diff = pipeline::diff::diff::compute(original, modified).expect("a diff");
    let alignment = pipeline::diff::diff::align(diff, original, modified).expect("alignment");
    let file = file_types::File::unchanged_path(
        file_types::RepoPath::new("test.rs", Path::new("/repo")),
        file_types::Revs::worktree_against(file_types::Oid::new("abc")),
    );
    pipeline::diff::DiffContent::Diff(pipeline::diff::Diff { file, alignment })
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
