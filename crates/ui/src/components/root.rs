//! What sits above the interface: the values that last as long as the session.

use std::rc::Rc;

use loom::{Node, Scope, component, rsx};

use super::app::{App, AppProps, FlowContext, FlowContextProps};
use super::context::{
    DiffStore, DiffStoreContext, DiffStoreContextProps, FileListStore, FileListStoreContext,
    FileListStoreContextProps, ThemeContext, ThemeContextProps,
};
use crate::app::Flow;
use crate::theme::Theme;

/// The mount point. The session owns the theme, the two stores the workers
/// write, and the callback that takes the program out; this offers all four to
/// whoever below asks for them.
///
/// Nothing is handed down as a prop — `App` takes none — so a component that
/// reads a store subscribes to it rather than waiting for a parent to pass on
/// what it read.
#[component]
pub fn Root(
    scope: &mut Scope,
    theme: Rc<Theme>,
    diff_store: DiffStore,
    file_list_store: FileListStore,
    on_flow: Rc<dyn Fn(Flow)>,
) -> Node {
    let _ = scope;

    rsx! {
        ThemeContext {
            value: Rc::clone(theme),
            DiffStoreContext {
                value: diff_store.clone(),
                FileListStoreContext {
                    value: file_list_store.clone(),
                    FlowContext {
                        value: Rc::clone(on_flow),
                        App {}
                    }
                }
            }
        }
    }
}
