//! The hatching drawn where a side has no line.

use std::rc::Rc;

use loom::{Basis, Canvas, CanvasProps, Layout, Node, Scope, component, rsx, use_context};

use super::context::ThemeContext;
use crate::cells;

/// The symbol repeated where a side has no line.
///
/// It is visibly not content, and it slants, so a block of them reads as a
/// gap.
const HATCH: &str = "╱";

/// One cell tall, full width, `╱` repeated.
#[component]
pub fn Filler(scope: &mut Scope) -> Node {
    let theme = use_context::<ThemeContext>(scope);
    let style = theme.normal.patch(theme.filler);

    rsx! {
        Canvas {
            layout: Layout { basis: Basis::Length(1), shrink: 0, ..Default::default() },
            paint: Rc::new(move |paint: &mut loom::Paint<'_>| {
                let area = paint.area();
                cells::fill_repeat_pattern(paint.cells(), area, HATCH, style);
            }),
            ..
        }
    }
}
