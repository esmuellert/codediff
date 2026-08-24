//! One diff, one version per row: two gutters and one text column.

use std::rc::Rc;

use align::{DiffVersion, Slot};
use file_types::DiffType;
use loom::{
    Basis, Bubble, Canvas, CanvasProps, Column, ColumnProps, Layout, Listeners, Mouse, Node, Row,
    RowProps, Scope, capture_pointer, component, release_pointer, rsx, use_context,
    use_layout_effect, use_ref, use_sync_external_store,
};

use super::context::{
    CursorContext, DiffStoreContext, FirstCellContext, PaneContext, ScreenMapCellContext,
    SelectionContext, SyntaxOnContext, ThemeContext, ViewLinesContext,
};
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
/// Selection is read from context and changed through `on_select`.
#[component]
pub fn Inline(scope: &mut Scope, on_select: Rc<dyn Fn(Option<Selection>)>) -> Node {
    let theme = use_context::<ThemeContext>(scope);
    let view_lines = use_context::<ViewLinesContext>(scope);
    let cursor = use_context::<CursorContext>(scope);
    let first_cell = use_context::<FirstCellContext>(scope);
    let syntax_on = use_context::<SyntaxOnContext>(scope);
    let diffs = use_context::<DiffStoreContext>(scope);
    let selection = use_context::<SelectionContext>(scope);
    let pane = use_context::<PaneContext>(scope);
    let map = use_context::<ScreenMapCellContext>(scope);
    // The workers fill the store; this subscribes rather than being handed
    // what they produced.
    let reading = use_sync_external_store(scope, &diffs);

    // Where a press landed, kept until a drag makes a selection of it. A
    // click that never drags selects nothing, so this is not a selection.
    let pending = use_ref(scope, || None::<Pos>);
    let node = use_ref(scope, || None::<loom::NodeHandle>);
    // How wide the two gutters turned out, written by the body below. The
    // effect is declared here, before the early returns, because a hook may
    // not run behind a condition.
    let gutter_cells = use_ref(scope, || 0u16);

    let filling = Rc::clone(&map);
    use_layout_effect(scope, loom::Always, move || {
        let Some(pane) = pane else { return };
        let Some(node) = *node.current() else { return };
        let gutters = *gutter_cells.current();
        let area = node.area();
        // A screen too small for two panes draws one of them instead, and the
        // one it left out has no rectangle. Nothing is under a mouse there,
        // so nothing is recorded.
        if area.is_empty() {
            return;
        }
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

    let Some(content) = reading.content.as_ref() else {
        return Node::Empty;
    };
    let pipeline::file::DiffContent::Diff(diff) = content.as_ref() else {
        return Node::Empty;
    };
    let alignment = &diff.alignment;

    // How the syntax worker has coloured the file so far, or nothing at all
    // when the reader has turned colour off. The borrow is held for the whole
    // body, because the spans are borrowed from it rather than copied.
    let colours = reading.colours.borrow();
    let spans = if syntax_on {
        crate::components::colour::spans_for(&diff.file, &colours)
    } else {
        syntax::Spans::Off
    };

    let original_width = gutter_width(alignment.lines(DiffVersion::Original).len() as u32);
    let modified_width = gutter_width(alignment.lines(DiffVersion::Modified).len() as u32);
    let gutters = original_width + modified_width;
    *gutter_cells.current() = gutters;

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
    let start = Rc::clone(on_select);
    let drag = Rc::clone(on_select);
    let end = Rc::clone(on_select);
    let listeners = Listeners::new()
        .on_mouse_down(move |mouse| {
            *pending.current() = at(mouse);
            if pending.current().is_some() {
                capture_pointer();
            }
            start(None);
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
                drag(Some(made));
            }
            Bubble::Stop
        })
        .on_mouse_up(move |_| {
            release_pointer();
            *pending.current() = None;
            // A drag that came back to where it started selects nothing.
            if selection.is_some_and(|held| held.is_empty()) {
                end(None);
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
            ref: Some(node),
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
