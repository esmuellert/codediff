//! What sits above the interface: the values that last as long as the session.

use std::rc::Rc;

use loom::{Node, Scope, component, rsx};

use super::app::{App, AppProps, FlowContext, FlowContextProps};
use super::context::{
    CursorCellContext, CursorCellContextProps, DiffStore, DiffStoreContext,
    DiffStoreContextProps, FileListStore, FileListStoreContext, FileListStoreContextProps,
    ScreenMapCellContext, ScreenMapCellContextProps, SelectionCellContext,
    SelectionCellContextProps, ThemeContext, ThemeContextProps, ViewLinesCellContext,
    ViewLinesCellContextProps,
};
use crate::app::Flow;
use crate::theme::Theme;

/// The mount point. Session provides the theme, the two stores, the flow
/// callback, and the observation cells for tests.
#[component]
pub fn Root(
    scope: &mut Scope,
    theme: Rc<Theme>,
    diff_store: DiffStore,
    file_list_store: FileListStore,
    on_flow: Rc<dyn Fn(Flow)>,
    cursor_cell: Rc<std::cell::Cell<u32>>,
    view_lines_cell: Rc<std::cell::Cell<u32>>,
    selection_cell: Rc<std::cell::RefCell<Option<crate::state::selection::Selection>>>,
    screen_map_cell: Rc<std::cell::RefCell<crate::screen_map::ScreenMap>>,
) -> Node {
    let _ = scope;

    rsx! {
        SelectionCellContext {
            value: Rc::clone(selection_cell),
            ScreenMapCellContext {
                value: Rc::clone(screen_map_cell),
                CursorCellContext {
                    value: Rc::clone(cursor_cell),
                    ViewLinesCellContext {
                        value: Rc::clone(view_lines_cell),
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
            }
        }
    }
}
