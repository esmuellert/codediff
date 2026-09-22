//! A line marking an omitted unchanged region.

use std::rc::Rc;

use loom::{Basis, Canvas, CanvasProps, Layout, Node, Paint, Scope, component, rsx, use_context};
use ratatui::{layout::Rect, style::Style};

use super::context::Ui;
use crate::view::terminal_lines::TerminalLine;

pub(crate) fn is_fold_marker(original: &TerminalLine, modified: &TerminalLine) -> bool {
    matches!(
        (original, modified),
        (TerminalLine::Filler, TerminalLine::Filler)
    )
}

#[component]
pub(crate) fn FoldMarker(scope: &mut Scope) -> Node {
    let theme = use_context::<Ui>(scope).theme;
    let style = theme.normal.patch(theme.line_number);

    rsx! {
        Canvas {
            layout: Layout { grow: 1, basis: Basis::Length(1), shrink: 0, ..Default::default() },
            paint: Rc::new(move |paint: &mut loom::Paint<'_>| {
                paint_fold_marker(paint, paint.area(), style);
            }),
            ..
        }
    }
}

fn paint_fold_marker(paint: &mut Paint<'_>, area: Rect, style: Style) {
    for y in area.y..area.bottom() {
        for x in area.x..area.right() {
            paint.set(x, y, "·", style);
        }
    }
}
