//! One diff, one version per row: two gutters and one text column.

use std::rc::Rc;

use align::{DiffVersion, Slot};
use file_types::DiffType;
use loom::{
    Basis, Bubble, Canvas, CanvasProps, Column, ColumnProps, Layout, Listeners, Mouse, Node, Row,
    RowProps, Scope, capture_pointer, component, release_pointer, rsx, use_context, use_ref,
};

use super::context::Ui;
use super::{CodeText, CodeTextProps, Gutter, GutterProps, clip_to_line, gutter_width, row_styles};
use crate::cells;
use crate::components::selection::{Pos, Selection, SelectionColumn};

/// How narrow the text column may get before the pane refuses to draw.
const MIN_TEXT: u16 = 4;

/// One diff read down a single column, both versions in turn.
///
/// The empty gutter says which version a row shows: no modified number means
/// the line was deleted, no original number means it was inserted.
///
/// No props: the selection is read from context and changed through the
/// setter `App` left in `context`.
#[component]
pub fn Inline(scope: &mut Scope) -> Node {
    let ctx = use_context::<Ui>(scope);
    let theme = &ctx.theme;
    let view_lines = &ctx.diff_view_lines;
    let cursor = ctx.diff_cursor;
    let first_cell = ctx.first_cell;
    let selection = ctx.selection;
    let set_sel = ctx.set_selection;

    // Where a press landed, kept until a drag makes a selection of it. A
    // click that never drags selects nothing, so this is not a selection.
    let pending = use_ref(scope, || None::<Pos>);

    let Some(content) = ctx.diff.as_ref() else {
        return Node::Empty;
    };
    let pipeline::file::DiffContent::Diff(diff) = content.as_ref() else {
        return Node::Empty;
    };
    let alignment = &diff.alignment;

    // How the syntax worker has coloured the file so far. The borrow is held
    // for the whole body, because the spans are borrowed from it rather than
    // copied.
    let colours = ctx.colours.borrow();
    let spans = crate::components::colour::spans_for(&diff.file, &colours);

    let original_width = gutter_width(alignment.lines(DiffVersion::Original).len() as u32);
    let modified_width = gutter_width(alignment.lines(DiffVersion::Modified).len() as u32);
    let gutters = original_width + modified_width;

    // Where in the file a pointer landed, or `None` over either gutter.
    let top = view_lines.start;
    let last = view_lines.end.saturating_sub(1);
    let at = move |mouse: Mouse| -> Option<Pos> {
        let x = mouse.local.x.checked_sub(gutters)?;
        let line = (top + u32::from(mouse.local.y)).min(last);
        Some(Pos::new(line, first_cell + u32::from(x)))
    };

    // The pointer is captured on the way down, so a drag that leaves the
    // column keeps arriving until the button comes up. The press itself is
    // passed on, so that clicking a diff also moves the focus into it.

    let listeners = Listeners::new()
        .on_mouse_down(move |mouse| {
            *pending.current() = at(mouse);
            if pending.current().is_some() {
                capture_pointer();
            }
            if let Some(s) = set_sel { s(&|_| None); }
            Bubble::Continue
        })
        .on_mouse_move(move |mouse| {
            // Without a button this is the pointer passing over, which
            // selects nothing.
            if mouse.button.is_some()
                && let Some(anchor) = *pending.current()
                && let Some(pos) = at(mouse)
            {
                let mut made = Selection::start(SelectionColumn::Only, anchor);
                made.update(pos);
                if let Some(s) = set_sel { s(&move |_| Some(made)); }
            }
            Bubble::Stop
        })
        .on_mouse_up(move |_| {
            release_pointer();
            *pending.current() = None;
            // A drag that came back to where it started selects nothing.
            if selection.is_some_and(|held| held.is_empty()) {
                if let Some(s) = set_sel { s(&|_| None); }
            }
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
                theme,
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
