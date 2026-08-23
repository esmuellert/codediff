//! One line number, right-aligned with one space after it.

use loom::{Basis, Canvas, CanvasProps, Layout, Node, Scope, component, rsx};
use ratatui::style::Style;

use crate::cells;

/// One line number, right-aligned with one space before the text.
#[component]
pub fn Gutter(scope: &mut Scope, number: u32, width: u16, style: Style) -> Node {
    let _ = scope;
    let (number, width, style) = (*number, *width, *style);

    rsx! {
        Canvas {
            layout: Layout { basis: Basis::Length(width), shrink: 0, ..Default::default() },
            paint: std::rc::Rc::new(move |brush: &mut loom::Paint<'_>| {
                let area = brush.area();
                cells::fill(brush.cells(), area, style);
                let label = number.to_string();
                let offset = area
                    .width
                    .saturating_sub(1)
                    .saturating_sub(label.chars().count() as u16);
                cells::write(brush.cells(), area, offset, &label, style);
            }),
            ..
        }
    }
}
