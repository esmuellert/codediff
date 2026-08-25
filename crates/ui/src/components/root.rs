//! What sits above the interface: the values that last as long as the session.

use std::cell::RefCell;
use std::rc::Rc;

use file_types::File;
use loom::{Node, Scope, component, rsx};

use super::app::{App, AppProps};
use super::context::{Context, Observed, ObservedCtx, ObservedCtxProps, Ui, UiProps};
use crate::theme::Theme;

#[component]
pub fn Root(
    scope: &mut Scope,
    theme: Rc<Theme>,
    repo: Option<Rc<std::path::Path>>,
    diff: Option<Rc<pipeline::file::DiffContent>>,
    diff_version: syntax::Version,
    colours: Rc<RefCell<syntax::Store>>,
    files: Rc<Vec<File>>,
    observed: Rc<Observed>,
) -> Node {
    let _ = scope;

    rsx! {
        ObservedCtx {
            value: Rc::clone(observed),
            Ui {
                value: Context {
                    theme: Rc::clone(theme),
                    repo: repo.clone(),
                    diff: diff.clone(),
                    diff_version: *diff_version,
                    colours: Rc::clone(colours),
                    files: Rc::clone(files),
                    ..Context::default()
                },
                App {}
            }
        }
    }
}
