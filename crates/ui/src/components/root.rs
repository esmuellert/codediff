//! What sits above the interface: the values that last as long as the session.

use std::cell::RefCell;
use std::rc::Rc;

use loom::{Node, Scope, component, rsx};

use super::context::{
    ColoursContext, ColoursContextProps, SyntaxOnContext, SyntaxOnContextProps, ThemeContext,
    ThemeContextProps,
};
use super::{App, AppProps};
use crate::screen_map::ScreenMap;
use crate::state::View;
use crate::theme::Theme;

/// The theme and the worker store, provided once, above everything.
///
/// They outlast every frame, so they are offered here rather than held by the
/// interface that reads them.
#[component]
pub fn Root(
    scope: &mut Scope,
    view: Rc<RefCell<View>>,
    notice: Option<Rc<str>>,
    map: Rc<RefCell<ScreenMap>>,
    theme: Rc<Theme>,
    colours: Rc<std::cell::RefCell<syntax::Store>>,
    syntax_on: bool,
) -> Node {
    let _ = scope;

    rsx! {
        ThemeContext {
            value: Rc::clone(theme),
            ColoursContext {
                value: Rc::clone(colours),
                SyntaxOnContext {
                    value: *syntax_on,
                    App {
                        view: Rc::clone(view),
                        notice: notice.clone(),
                        map: Rc::clone(map),
                    }
                }
            }
        }
    }
}
