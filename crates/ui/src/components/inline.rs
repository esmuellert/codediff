//! One diff, one version per row: two gutters and one text column.

use std::rc::Rc;

use align::{DiffVersion, Slot};
use file_types::DiffType;
use loom::{
    Basis, Bubble, Canvas, CanvasProps, Column, ColumnProps, Layout, Listeners, Mouse, Node, Row,
    RowProps, Scope, capture_pointer, component, release_pointer, rsx, use_context, use_state,
    use_sync_external_store,
};

use super::context::{
    CursorContext, DiffStoreContext, FirstCellContext, SyntaxOnContext, ThemeContext,
    ViewLinesContext,
};
use super::{CodeText, CodeTextProps, Gutter, GutterProps, clip_to_line, gutter_width, row_styles};
use crate::cells;
use crate::state::selection::{Pos, Selection, SelectionColumn};

/// How narrow the text column may get before the pane refuses to draw.
const MIN_TEXT: u16 = 4;

/// One diff read down a single column, both versions in turn.
///
/// The empty gutter says which version a row shows: no modified number means
/// the line was deleted, no original number means it was inserted.
///
/// The text selection is this component's own state; no parent needs it.
#[component]
pub fn Inline(scope: &mut Scope) -> Node {
    let theme = use_context::<ThemeContext>(scope);
    let view_lines = use_context::<ViewLinesContext>(scope);
    let cursor = use_context::<CursorContext>(scope);
    let first_cell = use_context::<FirstCellContext>(scope);
    let syntax_on = use_context::<SyntaxOnContext>(scope);
    let diffs = use_context::<DiffStoreContext>(scope);
    // The workers fill the store; this subscribes rather than being handed
    // what they produced.
    let reading = use_sync_external_store(scope, &diffs);

    // Where a drag started and where it has reached. There is one column and
    // one file, so nothing above this component has any use for it.
    let (selection, set_selection) = use_state(scope, || None::<Selection>);

        let Some(content) = reading.content.as_ref() else { return Node::Empty };
    let pipeline::file::DiffContent::Diff(diff) = content.as_ref() else {
        return Node::Empty;
    };
    let alignment = &diff.alignment;

    // How the syntax worker has coloured the file so far, or nothing at all
    // when the reader has turned colour off.
    let spans = if syntax_on {
        crate::state::buffer::colour::spans_for(&diff.file, &reading.colours)
    } else {
        syntax::Spans::Off
    };

    let original_width = gutter_width(alignment.lines(DiffVersion::Original).len() as u32);
    let modified_width = gutter_width(alignment.lines(DiffVersion::Modified).len() as u32);
    let gutters = original_width + modified_width;

    // Where in the file a pointer landed, or `None` over either gutter.
    let top = view_lines.start;
    let at = move |mouse: Mouse| -> Option<Pos> {
        let x = mouse.local.x.checked_sub(gutters)?;
        Some(Pos::new(top + u32::from(mouse.local.y), first_cell + u32::from(x)))
    };

    // The pointer is captured on the way down, so a drag that leaves the
    // column keeps arriving until the button comes up.
    let listeners = Listeners::new()
        .on_mouse_down(move |mouse| {
            if let Some(pos) = at(mouse) {
                capture_pointer();
                set_selection(&move |_| Some(Selection::start(SelectionColumn::Only, pos)));
            }
            Bubble::Stop
        })
        .on_mouse_move(move |mouse| {
            // Without a button this is the pointer passing over, which
            // selects nothing.
            if mouse.button.is_some()
                && let Some(pos) = at(mouse)
            {
                set_selection(&move |held: Option<Selection>| {
                    let mut held = held?;
                    held.update(pos);
                    Some(held)
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
                (Slot::Line(number), _) => (DiffVersion::Modified, number),
                (_, Slot::Line(number)) => (DiffVersion::Original, number),
                (Slot::Filler, Slot::Filler) => {
                    // Inline gives each version a row of its own, so a row
                    // with neither cannot arise; the match must be exhaustive
                    // all the same.
                    let style = theme.normal;
                    return rsx! {
                        Canvas {
                            key: offset,
                            layout: Layout {
                                basis: Basis::Length(1),
                                shrink: 0,
                                ..Default::default()
                            },
                            paint: Rc::new(move |paint: &mut loom::Paint<'_>| {
                                let area = paint.area();
                                cells::fill(paint.cells(), area, style);
                            }),
                            ..
                        }
                    };
                }
            };

            let (unchanged, changed, numbers) = row_styles(
                &theme,
                line.kind,
                diff_version,
                alignment.moved(diff_version, number).is_some(),
                index == cursor,
            );

            let diff: Rc<[std::ops::Range<u32>]> = alignment
                .spans(diff_version, number)
                .into_iter()
                .map(|span| span.bytes)
                .collect();
            let syntax: Rc<[syntax::Span]> = Rc::from(spans.line(diff_version, number));
            let text: Rc<str> = Rc::from(alignment.line(diff_version, number).unwrap_or(""));

            rsx! {
                Row {
                    key: offset,
                    layout: Layout { basis: Basis::Length(1), shrink: 0, ..Default::default() },
                    ..,
                    // Which gutter is empty is what marks the row deleted or
                    // inserted, and the blank wears the row's own colour, so
                    // the change runs from the edge into the code.
                    Gutter {
                        number: line.original.line(),
                        style: numbers,
                        blank: unchanged,
                        width: original_width,
                    }
                    Gutter {
                        number: line.modified.line(),
                        style: numbers,
                        blank: unchanged,
                        width: modified_width,
                    }
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
            // Filled below the end of the document, so the column's end reads
            // as the file's end rather than as unpainted screen.
            layout: Layout {
                grow: 1,
                min_width: gutters + MIN_TEXT,
                fill: Some(theme.normal),
                ..Default::default()
            },
            listeners: listeners,
            ..,
            { rows }
        }
    }
}
