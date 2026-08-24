//! One diff, one version per row: two gutters and one text column.

use std::rc::Rc;

use align::{DiffVersion, Slot};
use file_types::DiffType;
use loom::{
    Basis, Bubble, Column, ColumnProps, Element, Layout, Listeners, Mouse, Node, Row,
    RowProps, Scope, capture_pointer, component, release_pointer, rsx, use_context, use_state,
};

use super::context::{
    CursorContext, DiffDataContext, FirstCellContext, ThemeContext, ViewLinesContext,
};
use super::{
    CodeText, CodeTextProps, Gutter, GutterProps, clip_to_line, gutter_width, row_styles,
};
use crate::state::selection::{Pos, Selection, SelectionColumn};

/// How narrow the text column may get before the pane refuses to draw.
const MIN_TEXT: u16 = 4;

/// One diff, one version per row.
///
/// The empty gutter says which version: no modified number means the line was
/// deleted, no original number means it was inserted.
#[component]
pub fn Inline(scope: &mut Scope) -> Node {
    let theme = use_context::<ThemeContext>(scope);
    let diff_data = use_context::<DiffDataContext>(scope);
    let view_lines = use_context::<ViewLinesContext>(scope);
    let cursor = use_context::<CursorContext>(scope);
    let first_cell = use_context::<FirstCellContext>(scope);

    let loaded = loom::use_sync_external_store(scope, &diff_data);
    let (selection, set_selection) = use_state(scope, || None::<Selection>);

    let Some(file) = loaded.diff.clone() else { return Node::Empty };
    let alignment = &file.alignment;

    let original_width = gutter_width(alignment.lines(DiffVersion::Original).len() as u32);
    let modified_width = gutter_width(alignment.lines(DiffVersion::Modified).len() as u32);
    let gutters = original_width + modified_width;

    let spans = if loaded.syntax_on {
        crate::state::buffer::colour::spans_diff(&file, &loaded.colours)
    } else {
        syntax::Spans::Off
    };

    let top = view_lines.start;
    let at = move |mouse: Mouse| -> Option<Pos> {
        let x = mouse.local.x.checked_sub(gutters)?;
        Some(Pos::new(top + u32::from(mouse.local.y), first_cell + u32::from(x)))
    };

    let listeners = Listeners::new()
        .on_mouse_down(move |mouse| {
            if let Some(pos) = at(mouse) {
                capture_pointer();
                set_selection(&move |_| Some(Selection::start(SelectionColumn::Only, pos)));
            }
            Bubble::Stop
        })
        .on_mouse_move(move |mouse| {
            if mouse.button.is_some()
                && let Some(pos) = at(mouse)
            {
                set_selection(&move |held: Option<Selection>| {
                    held.map(|mut held| {
                        held.update(pos);
                        held
                    })
                });
            }
            Bubble::Stop
        })
        .on_mouse_up(move |_| {
            release_pointer();
            Bubble::Stop
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
            layout: Layout { grow: 1, min_width: gutters + MIN_TEXT, ..Default::default() },
            listeners: listeners,
            ..,
            { rows }
        }
    }
}

