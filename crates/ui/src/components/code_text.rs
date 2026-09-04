//! One line of code with diff and syntax highlighting.

use std::ops::Range;
use std::rc::Rc;

use loom::{Basis, Canvas, CanvasProps, Layout, Node, Scope, component, rsx, use_context};
use ratatui::style::Style;

use super::cells::{self, Ink};
use super::context::Ui;

const TAB_WIDTH: u8 = 4;

fn width_in_cells(text: &str) -> u32 {
    line_index::LineIndex::new(text, TAB_WIDTH).width().0
}

pub(super) fn longest_line_cells(lines: &[String]) -> u32 {
    lines
        .iter()
        .map(|line| width_in_cells(line))
        .max()
        .unwrap_or(0)
}

#[component]
pub fn CodeText(
    scope: &mut Scope,
    text: Rc<str>,
    first_cell: u32,
    diff: Rc<[Range<u32>]>,
    fill_from: Option<u32>,
    empty_markers: Rc<[u32]>,
    syntax: Rc<[syntax::Span]>,
    unchanged_style: Style,
    changed_style: Style,
    selection: Option<Range<u32>>,
) -> Node {
    let theme = use_context::<Ui>(scope).theme;

    let text = Rc::clone(text);
    let first_cell = *first_cell;
    let diff = Rc::clone(diff);
    let fill_from = *fill_from;
    let empty_markers = Rc::clone(empty_markers);
    let syntax = Rc::clone(syntax);
    let unchanged_style = *unchanged_style;
    let changed_style = *changed_style;
    let selection = selection.clone();

    rsx! {
        Canvas {
            layout: Layout { grow: 1, basis: Basis::Length(1), shrink: 0, ..Default::default() },
            paint: Rc::new(move |paint: &mut loom::Paint<'_>| {
                let area = paint.area();
                cells::paint(
                    paint.cells(), area, &text,
                    TAB_WIDTH, first_cell,
                    Ink {
                        base: unchanged_style,
                        emphasis: changed_style,
                        spans: &diff,
                        fill_from,
                        empty_markers: &empty_markers,
                        syntax: &syntax,
                        code: &theme.code,
                    },
                );

                if let Some(ref selected) = selection {
                    for x in area.x..area.right() {
                        let col = first_cell + u32::from(x - area.x);
                        if selected.contains(&col)
                            && let Some(cell) = paint.cells().cell_mut((x, area.y))
                        {
                            cell.set_style(theme.selection);
                        }
                    }
                }
            }),
            ..
        }
    }
}
