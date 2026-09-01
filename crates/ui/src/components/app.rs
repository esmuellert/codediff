//! The whole interface.

use std::path::Path;
use std::rc::Rc;

use loom::{
    Bubble, Column, ColumnProps, Layout, Listeners, Node, Row, RowProps, Scope, component, rsx,
    use_exit,
};

use super::border::{Border, BorderProps};
use super::context::{UiProvider, UiProviderProps};
use super::diff_viewer::DiffViewer;
use super::explorer::Explorer;
use crate::services::diff::DiffService;
use crate::services::files::FilesService;
use crate::services::syntax::SyntaxService;
use crate::services::version_control::VersionControlService;

#[component]
pub fn App(
    scope: &mut Scope,
    cwd: Rc<Path>,
    file_service: Rc<FilesService>,
    diff_service: Rc<DiffService>,
    syntax_service: Rc<SyntaxService>,
    version_control_service: Rc<VersionControlService>,
) -> Node {
    let exit = use_exit(scope);
    let keys = Listeners::new().on_key(move |k| {
        if k == crokey::key!(q) {
            exit();
            Bubble::Stop
        } else {
            Bubble::Continue
        }
    });

    rsx! {
        Column {
            listeners: keys,
            layout: Layout { grow: 1, ..Default::default() },
            ..,
            UiProvider {
                cwd: Rc::clone(cwd),
                file_service: Rc::clone(file_service),
                diff_service: Rc::clone(diff_service),
                syntax_service: Rc::clone(syntax_service),
                version_control_service: Rc::clone(version_control_service),
                Row {
                    layout: Layout { grow: 1, ..Default::default() },
                    ..,
                    Border {
                        layout: Layout { basis: loom::Basis::Length(40), shrink: 1, ..Default::default() },
                        Explorer {}
                    }
                    Border {
                        layout: Layout { grow: 1, ..Default::default() },
                        DiffViewer {}
                    }
                }
            }
        }
    }
}
