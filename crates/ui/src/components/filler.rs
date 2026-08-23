//! The hatching drawn where a side has no line.

use loom::{Layout, Node, Scope, component, rsx};

use crate::cells;

/// The symbol repeated where a side has no line.
///
/// It is visibly not content, and it slants, so a block of them reads as a
/// gap.
const HATCH: &str = "╱";

/// A whole row of hatching, gutter included, so no line number is implied.
#[component]
pub fn Filler(scope: &mut Scope, style: ratatui::style::Style) -> Node {
    let _ = scope;
    let style = *style;

    rsx! {
        loom::Canvas {
            layout: Layout { basis: loom::Basis::Length(1), shrink: 0, ..Default::default() },
            paint: std::rc::Rc::new(move |brush: &mut loom::Paint<'_>| {
                let area = brush.area();
                cells::fill_repeat_pattern(brush.cells(), area, HATCH, style);
            }),
            ..
        }
    }
}
