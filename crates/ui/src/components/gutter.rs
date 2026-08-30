//! One line number, right-aligned in its own column.

use std::rc::Rc;

use loom::{Basis, Canvas, CanvasProps, Layout, Node, Scope, component, rsx};
use ratatui::style::Style;

use super::cells;

#[component]
pub fn Gutter(
    scope: &mut Scope,
    number: Option<u32>,
    style: Style,
    blank: Style,
    width: u16,
) -> Node {
    let number = *number;
    let style = *style;
    let blank = *blank;
    let width = *width;
    let _ = scope;

    rsx! {
        Canvas {
            layout: Layout { basis: Basis::Length(width), shrink: 0, ..Default::default() },
            paint: Rc::new(move |paint: &mut loom::Paint<'_>| {
                let area = paint.area();

                let Some(n) = number else {
                    cells::fill(paint.cells(), area, blank);
                    return;
                };

                cells::fill(paint.cells(), area, style);
                let label = n.to_string();
                let digits = label.chars().count() as u16;
                let at = area.width.saturating_sub(1).saturating_sub(digits);
                cells::write(paint.cells(), area, at, &label, style);
            }),
            ..
        }
    }
}
