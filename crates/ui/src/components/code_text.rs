//! One line of a file, on one row of the grid.

use std::ops::Range;
use std::rc::Rc;

use loom::{Canvas, CanvasProps, Layout, Node, Scope, component, rsx};
use ratatui::style::Style;

use crate::cells::{self, Ink};
use crate::theme::Code;

/// One line of a file, with the bytes that differ picked out and the
/// selection drawn over the top.
#[component]
pub fn CodeText(
    scope: &mut Scope,
    text: Rc<str>,
    diff: Rc<[Range<u32>]>,
    syntax: Rc<[syntax::Span]>,
    code: Rc<Code>,
    unchanged_style: Style,
    changed_style: Style,
    selection: Option<Range<u32>>,
    first_cell: u32,
    selected_style: Style,
) -> Node {
    let _ = scope;
    let text = Rc::clone(text);
    let diff = Rc::clone(diff);
    let syntax = Rc::clone(syntax);
    let code = Rc::clone(code);
    let (unchanged_style, changed_style) = (*unchanged_style, *changed_style);
    let (selection, first_cell, selected_style) = (selection.clone(), *first_cell, *selected_style);

    rsx! {
        Canvas {
            layout: Layout { grow: 1, ..Default::default() },
            paint: Rc::new(move |brush: &mut loom::Paint<'_>| {
                let area = brush.area();
                cells::paint(
                    brush.cells(),
                    area,
                    &text,
                    line_index::DEFAULT_TAB_WIDTH,
                    first_cell,
                    Ink {
                        base: unchanged_style,
                        emphasis: changed_style,
                        spans: &diff,
                        syntax: &syntax,
                        code: &code,
                    },
                );

                // The selection is painted over what the line already says,
                // so a selected change keeps its own colours underneath.
                if let Some(range) = &selection {
                    let from = area.x.saturating_add(range.start.min(u32::from(u16::MAX)) as u16);
                    let to = area
                        .x
                        .saturating_add(range.end.min(u32::from(u16::MAX)) as u16)
                        .min(area.right());
                    for x in from..to {
                        if let Some(cell) = brush.cells().cell_mut((x, area.y)) {
                            cell.set_style(cell.style().patch(selected_style));
                        }
                    }
                }
            }),
            ..
        }
    }
}
