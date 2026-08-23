//! One line number, right-aligned with one space after it.

use std::rc::Rc;

use loom::{Basis, Canvas, CanvasProps, Layout, Node, Scope, component, rsx};
use ratatui::style::Style;

use crate::cells;

/// One line number, or a blank column where the version has no line.
#[component]
pub fn Gutter(
    scope: &mut Scope,
    number: Option<u32>,
    style: Style,
    blank: Style,
    width: u16,
) -> Node {
    let _ = scope;
    let (number, style, blank, width) = (*number, *style, *blank, *width);

    rsx! {
        Canvas {
            layout: Layout { basis: Basis::Length(width), shrink: 0, ..Default::default() },
            paint: Rc::new(move |paint: &mut loom::Paint<'_>| {
                let area = paint.area();

                // Which gutter is empty is what marks a row deleted or
                // inserted, so the blank is filled in the row's own colour.
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
