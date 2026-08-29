//! The whole interface.

use std::path::Path;
use std::rc::Rc;

use loom::{
    Bubble, Column, ColumnProps, Layout, Listeners, Node, NodeHandle, Row, RowProps, Scope,
    component, rsx, use_context, use_exit, use_layout_effect, use_ref, use_state,
};

use super::context::{Context, Ui, UiProps, UiProvider, UiProviderProps};
use super::explorer::{Explorer, ExplorerProps};
use crate::services::file::FileService;

#[component]
pub fn App(scope: &mut Scope, cwd: Rc<Path>, file_service: Rc<FileService>) -> Node {
    let exit = use_exit(scope);
    let keys = Listeners::new().on_key(move |_| {
        exit();
        Bubble::Stop
    });

    rsx! {
        Column {
            listeners: keys,
            layout: Layout { grow: 1, ..Default::default() },
            ..,
            UiProvider {
                cwd: Rc::clone(cwd),
                file_service: Rc::clone(file_service),
                AppBody {}
            }
        }
    }
}

/// The layout inside the provider. Measures the body and updates view_lines
/// in context.
#[component]
fn AppBody(scope: &mut Scope) -> Node {
    let ctx = use_context::<Ui>(scope);
    let (rows, set_rows) = use_state(scope, || 0u32);

    let body = use_ref(scope, || None::<NodeHandle>);
    use_layout_effect(scope, loom::Always, move || {
        let area = body.current().map_or(ratatui::layout::Rect::ZERO, |node| node.area());
        set_rows(&move |_| u32::from(area.height));
    });

    rsx! {
        Ui {
            value: Context {
                view_lines: 0..rows,
                ..ctx.clone()
            },
            Row {
                ref: Some(body),
                layout: Layout { grow: 1, ..Default::default() },
                ..,
                Explorer { on_open: Rc::new(|_| {}) }
            }
        }
    }
}
