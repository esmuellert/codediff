//! One line of a file, on one row of the grid.

use std::ops::Range;
use std::rc::Rc;

use line_index::DEFAULT_TAB_WIDTH;
use loom::{Canvas, CanvasProps, Layout, Node, Scope, component, rsx, use_context};
use ratatui::style::Style;

use super::context::Ui;
use crate::cells::{self, Ink};

/// One line of a file, with the bytes that differ picked out and the
/// selection drawn over the top.
#[component]
pub fn CodeText(
    scope: &mut Scope,
    text: Rc<str>,
    diff: Rc<[Range<u32>]>,
    syntax: Rc<[syntax::Span]>,
    unchanged_style: Style,
    changed_style: Style,
    selection: Option<Range<u32>>,
) -> Node {
    let ctx = use_context::<Ui>(scope);
    let theme = Rc::clone(&ctx.theme);
    let first_cell = ctx.first_cell;

    let text = Rc::clone(text);
    let diff = Rc::clone(diff);
    let syntax = Rc::clone(syntax);
    let (unchanged_style, changed_style) = (*unchanged_style, *changed_style);
    let selection = selection.clone();

    rsx! {
        Canvas {
            layout: Layout { grow: 1, ..Default::default() },
            paint: Rc::new(move |paint: &mut loom::Paint<'_>| {
                let area = paint.area();
                cells::paint(
                    paint.cells(),
                    area,
                    &text,
                    DEFAULT_TAB_WIDTH,
                    first_cell,
                    Ink {
                        base: unchanged_style,
                        emphasis: changed_style,
                        spans: &diff,
                        syntax: &syntax,
                        code: &theme.code,
                    },
                );

                // Selection: replace the style of each selected cell.
                let Some(ref selected) = selection else { return };
                for x in area.x..area.right() {
                    let col = first_cell + u32::from(x - area.x);
                    if selected.contains(&col)
                        && let Some(cell) = paint.cells().cell_mut((x, area.y))
                    {
                        cell.set_style(theme.selection);
                    }
                }
            }),
            ..
        }
    }
}
