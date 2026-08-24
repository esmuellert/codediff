//! One diff, one version per row: two gutters and one text column.

use std::rc::Rc;

use align::{DiffVersion, Slot};
use file_types::DiffType;
use loom::{
    use_layout_effect, use_ref,
    Basis, Column, ColumnProps, Element, Layout, Node, Row,
    RowProps, Scope, component, rsx, use_context,
};

use super::context::{
    ColoursContext, CursorContext, FirstCellContext, PaneContext, ScreenMapContext, SyntaxOnContext,
    ThemeContext, ViewLinesContext,
};
use super::{
    CodeText, CodeTextProps, Gutter, GutterProps, clip_to_line, gutter_width, row_styles,
};
use crate::state::selection::SelectionColumn;

/// How narrow the text column may get before the pane refuses to draw.
const MIN_TEXT: u16 = 4;

/// One diff, one version per row.
///
/// The empty gutter says which version: no modified number means the line was
/// deleted, no original number means it was inserted.
#[component]
pub fn Inline(
    scope: &mut Scope,
    view: Rc<std::cell::RefCell<crate::state::View>>,
    buffer: crate::state::BufferId,
) -> Node {
    let theme = use_context::<ThemeContext>(scope);
    let colours = use_context::<ColoursContext>(scope);
    let syntax_on = use_context::<SyntaxOnContext>(scope);
    let map = use_context::<ScreenMapContext>(scope);
    let pane = use_context::<PaneContext>(scope);
    let view_lines = use_context::<ViewLinesContext>(scope);
    let cursor = use_context::<CursorContext>(scope);
    let _first_cell = use_context::<FirstCellContext>(scope);


    let store = colours.borrow();
    let read = view.borrow();
    let selection = read
        .selection
        .filter(|(owner, _)| Some(*owner) == pane)
        .map(|(_, held)| held);
    let held = read.buffer(*buffer);
    let Some(alignment) = held.alignment() else { return Node::Empty };
    let Some(file) = held.file() else { return Node::Empty };

    let original_width = gutter_width(alignment.lines(DiffVersion::Original).len() as u32);
    let modified_width = gutter_width(alignment.lines(DiffVersion::Modified).len() as u32);
    let _gutters = original_width + modified_width;

    let spans = if syntax_on {
        crate::state::buffer::colour::spans_for(file, &store)
    } else {
        syntax::Spans::Off
    };


    // The text column is the row less its two gutters. Recorded once layout
    // has decided, for whoever has to say what is under the mouse.
    let node = use_ref(scope, || None::<loom::NodeHandle>);
    let filling = Rc::clone(&map);
    let gutters = original_width + modified_width;
    use_layout_effect(scope, loom::Always, move || {
        let Some(pane) = pane else { return };
        let Some(node) = *node.current() else { return };
        let area = node.area();
        filling.borrow_mut().text_areas.push(crate::screen_map::TextArea {
            pane,
            column: SelectionColumn::Only,
            rect: ratatui::layout::Rect {
                x: area.x.saturating_add(gutters),
                width: area.width.saturating_sub(gutters),
                ..area
            },
        });
    });

    let rows: Vec<Node> = alignment
        .view_lines_from(DiffType::Inline, view_lines.start)
        .take(view_lines.len())
        .enumerate()
        .map(|(offset, line)| {
            let index = view_lines.start + offset as u32;

            // Which version this row shows. Only an unchanged line has both,
            // and then the two lines are the same text, so either answers.
            let (diff_version, number) = match (line.modified, line.original) {
                (Slot::Line(n), _) => (DiffVersion::Modified, n),
                (_, Slot::Line(n)) => (DiffVersion::Original, n),
                (Slot::Filler, Slot::Filler) => {
                    // Cannot occur in an inline diff, but the match must be
                    // exhaustive.
                    let style = theme.normal;
                    return <loom::Canvas as Element>::build(
                        loom::CanvasProps {
                            layout: Layout { basis: Basis::Length(1), shrink: 0, ..Default::default() },
                            paint: Rc::new(move |paint: &mut loom::Paint<'_>| {
                                let area = paint.area();
                                crate::cells::fill(paint.cells(), area, style);
                            }),
                            ..Default::default()
                        },
                        Some(loom::Key::from(offset)),
                    );
                }
            };

            let is_cursor = index == cursor;
            let (unchanged, changed, numbers) = row_styles(
                &theme,
                line.kind,
                diff_version,
                alignment.moved(diff_version, number).is_some(),
                is_cursor,
            );

            // Which gutter is empty is what marks the row deleted or
            // inserted, and the blank is filled in the row's own colour so
            // the change background runs edge to edge.
            let gutter = |slot: Slot, width: u16| -> Node {
                rsx! {
                    Gutter {
                        number: match slot { Slot::Line(n) => Some(n), Slot::Filler => None },
                        style: numbers,
                        blank: unchanged,
                        width: width,
                    }
                }
            };

            let diff: Rc<[std::ops::Range<u32>]> = alignment
                .spans(diff_version, number)
                .into_iter()
                .map(|s| s.bytes)
                .collect();
            let syntax: Rc<[syntax::Span]> = Rc::from(spans.line(diff_version, number));
            let text: Rc<str> = Rc::from(alignment.line(diff_version, number).unwrap_or(""));

            rsx! {
                Row {
                    key: offset,
                    layout: Layout { basis: Basis::Length(1), shrink: 0, ..Default::default() },
                    ..,
                    { gutter(line.original, original_width) }
                    { gutter(line.modified, modified_width) }
                    CodeText {
                        text: text,
                        diff: diff,
                        syntax: syntax,
                        unchanged_style: unchanged,
                        changed_style: changed,
                        selection: clip_to_line(selection.as_ref(), index),
                    }
                }
            }
        })
        .collect();

    rsx! {
        Column {
            ref: Some(node),
            layout: Layout {
                grow: 1,
                min_width: gutters + MIN_TEXT,
                fill: Some(theme.normal),
                ..Default::default()
            },
            ..,
            { rows }
        }
    }
}

