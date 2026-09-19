//! Application root component.

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
use crate::keybindings::{Action, KeyMap};
use crate::services::diff::DiffService;
use crate::services::files::FilesService;
use crate::services::syntax::SyntaxService;
use crate::services::version_control::VersionControlService;
use crate::services::watcher::WatcherService;

#[component]
pub fn App(
    scope: &mut Scope,
    cwd: Rc<Path>,
    config: Rc<config::Config>,
    keybindings: Rc<KeyMap>,
    files_service: Rc<FilesService>,
    diff_service: Rc<DiffService>,
    syntax_service: Rc<SyntaxService>,
    version_control_service: Rc<VersionControlService>,
    watcher_service: Rc<WatcherService>,
) -> Node {
    let exit = use_exit(scope);
    let keybindings_for_root = Rc::clone(keybindings);
    let explorer_width = config.ui.explorer_width;
    let keys = Listeners::new().on_key(move |k| {
        if keybindings_for_root.matches(Action::Quit, k) {
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
                config: Rc::clone(config),
                keybindings: Rc::clone(keybindings),
                files_service: Rc::clone(files_service),
                diff_service: Rc::clone(diff_service),
                syntax_service: Rc::clone(syntax_service),
                version_control_service: Rc::clone(version_control_service),
                watcher_service: Rc::clone(watcher_service),
                Row {
                    layout: Layout { grow: 1, ..Default::default() },
                    ..,
                    Border {
                        layout: Layout { basis: loom::Basis::Length(explorer_width), shrink: 1, ..Default::default() },
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
