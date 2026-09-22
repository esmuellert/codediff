use std::path::Path;
use std::rc::Rc;

use loom::testing::Harness;
use loom::{Node, Scope, component, rsx, use_ref};
use ui::Theme;
use ui::components::diff_viewer::{ViewState, ViewStateHistory};
use ui::components::diff_viewer_container::{DiffViewerContainer, DiffViewerContainerProps};
use ui::components::{Context, Ui};

#[component]
fn SideBySide(scope: &mut Scope, content: Rc<pipeline::diff::DiffContent>) -> Node {
    let view_states = use_ref(scope, ViewStateHistory::default);
    let state = use_ref(scope, ViewState::default);
    let key = content.file().path().as_str().to_owned();
    let active_key = use_ref(scope, || None::<String>);
    if active_key.current().as_deref() != Some(key.as_str()) {
        if let Some(previous) = active_key.current().clone() {
            view_states
                .current()
                .save(&previous, state.current().clone());
        }
        *state.current() = view_states.current().load(&key);
        *active_key.current() = Some(key);
    }
    rsx! {
        DiffViewerContainer {
            content: Some(Rc::clone(content)),
            view_layout: file_types::DiffType::SideBySide,
            view_state: state,
            wrap: false,
            compact: false,
            auto_focus: false,
        }
    }
}

fn content(original: &str, modified: &str) -> Rc<pipeline::diff::DiffContent> {
    let original_lines = [original];
    let modified_lines = [modified];
    let diff = pipeline::diff::compute(
        &original_lines,
        &modified_lines,
        pipeline::diff::Settings::default(),
    )
    .expect("a diff");
    let alignment =
        pipeline::diff::align(diff, &original_lines, &modified_lines).expect("an alignment");
    let file = file_types::File::unchanged_path(
        file_types::RepoPath::new("scroll.rs", Path::new("/repo")),
        file_types::Revs::worktree_against(file_types::Oid::new("abc")),
    );
    Rc::new(pipeline::diff::DiffContent::Diff(pipeline::diff::Diff {
        file,
        alignment,
    }))
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
