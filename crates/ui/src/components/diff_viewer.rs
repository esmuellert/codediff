//! Reads the diff from context and shows the right view.

use loom::{Node, Scope, component, rsx, use_context};

use super::context::Ui;
use super::side_by_side::SideBySide;
use super::single_file::SingleFile;
use super::welcome::Welcome;

#[component]
pub fn DiffViewer(scope: &mut Scope) -> Node {
    let ctx = use_context::<Ui>(scope);
    match ctx.diff.as_deref() {
        Some(pipeline::diff::DiffContent::Diff(_)) => rsx! { SideBySide {} },
        Some(pipeline::diff::DiffContent::SingleFile(_)) => rsx! { SingleFile {} },
        None => rsx! { Welcome {} },
    }
}
