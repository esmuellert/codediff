//! A hatch across a whole row, where one side has no line.

use std::rc::Rc;

use loom::{Basis, Canvas, CanvasProps, Layout, Node, Paint, Scope, component, rsx, use_context};
use ratatui::{layout::Rect, style::Style};

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
                paint_filler(paint, area, style);
            }),
            ..
        }
    }
}

pub(crate) fn paint_filler(paint: &mut Paint<'_>, area: Rect, style: Style) {
    for y in area.y..area.bottom() {
        for x in area.x..area.right() {
            paint.set(x, y, "╱", style);
        }
    }
}
