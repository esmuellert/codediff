use std::collections::HashMap;
use std::path::Path;
use std::rc::Rc;
use std::sync::Arc;

use loom::testing::Harness;
use loom::{Node, Scope, component, rsx, use_ref};
use ui::Theme;
use ui::components::diff_viewer::ScrollPosition;
use ui::components::diff_viewer_container::{DiffViewerContainer, DiffViewerContainerProps};
use ui::components::{Context, Ui};

#[component]
fn SideBySide(scope: &mut Scope, content: Arc<diff_service::diff::DiffContent>) -> Node {
    let scroll_positions = use_ref(scope, HashMap::<String, ScrollPosition>::new);
    let state = use_ref(scope, ScrollPosition::default);
    let key = content.file().path().as_str().to_owned();
    let active_key = use_ref(scope, || None::<String>);
    if active_key.current().as_deref() != Some(key.as_str()) {
        if let Some(previous) = active_key.current().clone() {
            scroll_positions
                .current()
                .insert(previous, state.current().clone());
        }
        *state.current() = scroll_positions
            .current()
            .get(&key)
            .cloned()
            .unwrap_or_default();
        *active_key.current() = Some(key);
    }
    rsx! {
        DiffViewerContainer {
            content: Some(Arc::clone(content)),
            colours: Arc::default(),
            viewer_mode: file_types::ViewerMode::SideBySide,
            scroll_position: state,
            wrap: false,
            compact: false,
            auto_focus: false,
        }
    }
}

fn content(original: &str, modified: &str) -> Arc<diff_service::diff::DiffContent> {
    let original_lines = [original];
    let modified_lines = [modified];
    let diff = diff_service::diff::compute(
        &original_lines,
        &modified_lines,
        diff_service::diff::Settings::default(),
    )
    .expect("a diff");
    let alignment =
        diff_service::diff::align(diff, &original_lines, &modified_lines).expect("an alignment");
    let file = file_types::File::unchanged_path(
        file_types::RepoPath::new("scroll.rs", Path::new("/repo")),
        file_types::Revs::worktree_against(file_types::CommitHash::new("abc")),
    );
    Arc::new(diff_service::diff::DiffContent::Diff(
        diff_service::diff::Diff { file, alignment },
    ))
}

#[test]
fn side_by_side_scrolls_to_each_panes_own_endpoint() {
    let original = "LEFT_LONG_START ".to_owned() + &"abcdefghij".repeat(4) + " LEFT_END";
    let modified = "RIGHT_LONG_START ".to_owned() + &"0123456789".repeat(8) + " RIGHT_END";
    let mut harness = Harness::new::<SideBySide>(
        SideBySideProps {
            content: content(&original, &modified),
        },
        60,
        4,
    )
    .provide::<Ui>(Context {
        theme: Rc::new(Theme::DARK),
        ..Context::default()
    });
    harness.force_draw().force_draw();
    harness.press(crokey::key!('$')).force_draw();

    let cells = harness.cells();
    let divider = (0..60)
        .find(|&column| {
            cells
                .cell((column, 0))
                .is_some_and(|cell| cell.symbol() == "│")
        })
        .expect("a side-by-side divider");
    let left = (0..divider)
        .filter_map(|column| cells.cell((column, 0)))
        .map(|cell| cell.symbol())
        .collect::<String>();
    let right = (divider.saturating_add(1)..60)
        .filter_map(|column| cells.cell((column, 0)))
        .map(|cell| cell.symbol())
        .collect::<String>();

    assert!(left.contains("LEFT_END"), "left side: {left:?}");
    assert!(right.contains("RIGHT_END"), "right side: {right:?}");
}
