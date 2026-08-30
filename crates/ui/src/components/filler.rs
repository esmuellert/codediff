//! A hatch across a whole row, where one side has no line.

use std::rc::Rc;

use loom::{Basis, Canvas, CanvasProps, Layout, Node, Scope, component, rsx, use_context};

use super::cells;
use super::context::Ui;

#[component]
pub fn Filler(scope: &mut Scope) -> Node {
    let theme = use_context::<Ui>(scope).theme;
    let style = theme.normal.patch(theme.filler);

    rsx! {
        Canvas {
            layout: Layout { grow: 1, basis: Basis::Length(1), shrink: 0, ..Default::default() },
            paint: Rc::new(move |paint: &mut loom::Paint<'_>| {
                let area = paint.area();
                cells::fill_repeat_pattern(paint.cells(), area, "╱", style);
            }),
            ..
        }
    }
}
