//! What sits above the interface: the values that last as long as the session.

use std::rc::Rc;

use loom::{Node, Scope, component, rsx};

use super::app::{App, AppProps};
use super::context::{
    Context, DiffStore, DiffStoreCtx, DiffStoreCtxProps, FileListStore, FileListStoreCtx,
    FileListStoreCtxProps, Observed, ObservedCtx, ObservedCtxProps, Ui, UiProps,
};
use crate::theme::Theme;

/// The mount point. Session provides the theme, the repository path, the two
/// stores, and the struct the frame writes to — which is also where the
/// callbacks only the session can answer are left.
///
/// The theme and the repository last as long as the session, so they go into
/// the context here with nothing else filled in. `App` reads them back and
/// provides the whole of it, which is what everything below sees.
#[component]
pub fn Root(
    scope: &mut Scope,
    theme: Rc<Theme>,
    repo: Option<Rc<std::path::Path>>,
    diff_store: DiffStore,
    file_list_store: FileListStore,
    observed: Rc<Observed>,
) -> Node {
    let _ = scope;

    rsx! {
        ObservedCtx {
            value: Rc::clone(observed),
            DiffStoreCtx {
                value: diff_store.clone(),
                FileListStoreCtx {
                    value: file_list_store.clone(),
                    Ui {
                        value: Context {
                            theme: Rc::clone(theme),
                            repo: repo.clone(),
                            ..Context::default()
                        },
                        App {}
                    }
                }
            }
        }
    }
}
