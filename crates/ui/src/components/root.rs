//! What sits above the interface: the values that last as long as the session.

use std::rc::Rc;

use loom::{Node, Scope, component, rsx};

use super::app::{App, AppProps, FlowContext, FlowContextProps};
use super::context::{
    CursorCellContext, CursorCellContextProps, DiffStore, DiffStoreContext,
    DiffStoreContextProps, FileListStore, FileListStoreContext, FileListStoreContextProps,
    LayoutCellContext, LayoutCellContextProps, OpenContext, OpenContextProps, RepoContext,
    RepoContextProps, ScreenMapCellContext, ScreenMapCellContextProps, SelectionCellContext,
    SelectionCellContextProps, ThemeContext, ThemeContextProps, ViewLinesCellContext,
    ViewLinesCellContextProps,
};
use crate::app::Flow;
use crate::theme::Theme;

/// The mount point. Session provides the theme, the repository path, the two
/// stores, the flow and open callbacks, and the observation cells for tests.
#[component]
pub fn Root(
    scope: &mut Scope,
    theme: Rc<Theme>,
    repo: Option<Rc<std::path::Path>>,
    diff_store: DiffStore,
    file_list_store: FileListStore,
    on_flow: Rc<dyn Fn(Flow)>,
    on_open: Rc<dyn Fn(file_types::File)>,
    cursor_cell: Rc<std::cell::Cell<u32>>,
    view_lines_cell: Rc<std::cell::Cell<u32>>,
    layout_cell: Rc<std::cell::Cell<file_types::DiffType>>,
    selection_cell: Rc<std::cell::RefCell<Option<crate::components::selection::Selection>>>,
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
                        LayoutCellContext {
                            value: Rc::clone(layout_cell),
                            ThemeContext {
                                value: Rc::clone(theme),
                                RepoContext {
                                    value: repo.clone(),
                                    DiffStoreContext {
                                        value: diff_store.clone(),
                                        FileListStoreContext {
                                            value: file_list_store.clone(),
                                            OpenContext {
                                                value: Rc::clone(on_open),
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
        }
    }
}
