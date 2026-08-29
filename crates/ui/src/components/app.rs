//! The whole interface.

use std::path::Path;
use std::rc::Rc;

use loom::{
    Bubble, Column, ColumnProps, Layout, Listeners, Node, NodeHandle, Row, RowProps, Scope,
    component, rsx, use_exit, use_layout_effect, use_ref, use_state,
};

use super::context::{UiProvider, UiProviderProps};
use super::explorer::{Explorer, ExplorerProps};
use crate::services::files::FilesService;

#[component]
pub fn App(scope: &mut Scope, cwd: Rc<Path>, file_service: Rc<FilesService>) -> Node {
    let (rows, set_rows) = use_state(scope, || 0u32);

    let exit = use_exit(scope);
    let keys = Listeners::new().on_key(move |k| {
        if k == crokey::key!(q) {
            exit();
            Bubble::Stop
        } else {
            Bubble::Continue
        }
    });

    let body = use_ref(scope, || None::<NodeHandle>);
    use_layout_effect(scope, loom::Always, move || {
        let area = body.current().map_or(ratatui::layout::Rect::ZERO, |node| node.area());
        set_rows(&move |_| u32::from(area.height));
    });

    rsx! {
        Column {
            listeners: keys,
            layout: Layout { grow: 1, ..Default::default() },
            ..,
            UiProvider {
                cwd: Rc::clone(cwd),
                file_service: Rc::clone(file_service),
                rows: rows,
                Row {
                    ref: Some(body),
                    layout: Layout { grow: 1, ..Default::default() },
                    ..,
                    Explorer { on_open: Rc::new(|_| {}) }
                }
            }
        }
    }
}
