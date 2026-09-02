//! Reads the diff from context and shows the right view.

use loom::{Node, Scope, component, rsx, use_context};

use super::context::Ui;
use super::side_by_side::SideBySide;
use super::welcome::Welcome;

#[component]
pub fn DiffViewer(scope: &mut Scope) -> Node {
    let ctx = use_context::<Ui>(scope);
    let has_diff = matches!(
        ctx.diff.as_deref(),
        Some(pipeline::diff::DiffContent::Diff(_))
    );

    if has_diff {
        rsx! { SideBySide {} }
    } else {
        rsx! { Welcome {} }
    }
}
