//! Two columns of one diff, and the divider between them.

use std::rc::Rc;

use align::{DiffVersion, Slot};
use file_types::DiffType;
use loom::{
    Basis, Bubble, Column, ColumnProps, Divider, DividerProps, Layout, Listeners, Mouse, Node, Row,
    RowProps, Scope, capture_pointer, component, release_pointer, rsx, use_context,
    use_layout_effect, use_ref, use_state, use_sync_external_store,
};

use super::context::{DiffStoreCtx, ObservedCtx, Ui};
use super::{
    CodeText, CodeTextProps, Filler, FillerProps, Gutter, GutterProps, clip_to_line, gutter_width,
    row_styles,
};
use crate::components::selection::{Pos, Selection, SelectionColumn};

/// How narrow a text column may get before the pane refuses to draw.
const MIN_TEXT: u16 = 4;

/// Two columns of one file, side by side, with a divider between them.
///
/// No props: the divider's position is this component's own state, and the
/// selection is read from context and changed through the setter `App` left
/// in `Observed`.
#[component]
pub fn SideBySide(scope: &mut Scope) -> Node {
    let ctx = use_context::<Ui>(scope);
    let theme = &ctx.theme;
    let view_lines = &ctx.view_lines;
    let cursor = ctx.cursor;
    let first_cell = ctx.first_cell;
    let selection = ctx.selection;
    let diffs = use_context::<DiffStoreCtx>(scope);
    let observed = use_context::<ObservedCtx>(scope);
    // The workers fill the store; this subscribes rather than being handed
    // what they produced.
    let reading = use_sync_external_store(scope, &diffs);

    // Where a press landed, kept until a drag makes a selection of it. A
    // click that never drags selects nothing, so this is not a selection.
    let pending = use_ref(scope, || None::<(SelectionColumn, Pos)>);
    // Percent of the width the left column takes.
    let (divider, _set_divider) = use_state(scope, || 50u16);

    // The divider is taken off the top before dividing, so widening the pane
    // by one column widens a column rather than the divider. That needs this
    // component's own width, which layout knows and the render body does not.
    // The effect is declared here, before the early returns, because a hook
    // may not run behind a condition.
    let node = use_ref(scope, || None::<loom::NodeHandle>);
    let (width, set_width) = use_state(scope, || 0u16);
    use_layout_effect(scope, loom::Always, move || {
        let now = node.current().map_or(0, |node| node.area().width);
        set_width(&move |_| now);
    });

    let Some(content) = reading.content.as_ref() else {
        return Node::Empty;
    };
    let pipeline::file::DiffContent::Diff(diff) = content.as_ref() else {
        return Node::Empty;
    };
    let alignment = &diff.alignment;

    // How the syntax worker has coloured the file so far. The borrow is held
    // for the whole body, because the spans are borrowed from it rather than
    // copied.
    let colours = reading.colours.borrow();
    let spans = crate::components::colour::spans_for(&diff.file, &colours);

    // Collected once and read by both columns, so the two cannot disagree
    // about what line they are on.
    let lines: Vec<align::ViewLine> = alignment
        .view_lines_from(DiffType::SideBySide, view_lines.start)
        .take(view_lines.len())
        .collect();

    let original_width = gutter_width(alignment.lines(DiffVersion::Original).len() as u32);
    let modified_width = gutter_width(alignment.lines(DiffVersion::Modified).len() as u32);

    let column = |diff_version: DiffVersion, layout: Layout| -> Node {
        let gutter = gutter_width(alignment.lines(diff_version).len() as u32);
        let which = match diff_version {
            DiffVersion::Original => SelectionColumn::Original,
            DiffVersion::Modified => SelectionColumn::Modified,
        };

        // Where in the file a pointer landed, or `None` over the gutter.
        let top = view_lines.start;
        let last = view_lines.end.saturating_sub(1);
        let at = move |mouse: Mouse| -> Option<Pos> {
            let x = mouse.local.x.checked_sub(gutter)?;
            let line = (top + u32::from(mouse.local.y)).min(last);
            Some(Pos::new(line, first_cell + u32::from(x)))
        };

        // The pointer is captured on the way down, so a drag that leaves the
        // column keeps arriving until the button comes up. The press itself
        // is passed on, so that clicking a diff also moves the focus into it.
        let held = selection;
        let start = Rc::clone(&observed);
        let drag = Rc::clone(&observed);
        let end = Rc::clone(&observed);
        let listeners = Listeners::new()
            .on_mouse_down(move |mouse| {
                *pending.current() = at(mouse).map(|pos| (which, pos));
                if pending.current().is_some() {
                    capture_pointer();
                }
                start.select(None);
                Bubble::Continue
            })
            .on_mouse_move(move |mouse| {
                // Without a button this is the pointer passing over, which
                // selects nothing.
                if mouse.button.is_some()
                    && let Some((column, anchor)) = *pending.current()
                    && let Some(pos) = at(mouse)
                {
                    let mut made = Selection::start(column, anchor);
                    made.update(pos);
                    drag.select(Some(made));
                }
                Bubble::Stop
            })
            .on_mouse_up(move |_| {
                release_pointer();
                *pending.current() = None;
                // A drag that came back to where it started selects nothing.
                if held.is_some_and(|held| held.is_empty()) {
                    end.select(None);
                }
                Bubble::Stop
            });

        // A selection lives in the column it was drawn in; the other side
        // shows none of it.
        let mine = selection.filter(|held| held.column == which);

        let rows: Vec<Node> = lines
            .iter()
            .enumerate()
            .map(|(offset, line)| {
                let index = view_lines.start + offset as u32;
                let slot = match diff_version {
                    DiffVersion::Original => line.original,
                    DiffVersion::Modified => line.modified,
                };

                let Slot::Line(number) = slot else {
                    // No line on this side at all, so the whole width is
                    // hatched and no line number is implied.
                    return rsx! { Filler { key: offset } };
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
                        Gutter {
                            number: Some(number),
                            style: numbers,
                            blank: unchanged,
                            width: gutter,
                        }
                        CodeText {
                            text: text,
                            diff: diff,
                            syntax: syntax,
                            unchanged_style: unchanged,
                            changed_style: changed,
                            selection: clip_to_line(mine.as_ref(), index),
                        }
                    }
                }
            })
            .collect();

        rsx! {
            Column {
                // Filled below the end of the document, so the two sides' ends
                // stay visually comparable.
                layout: Layout { fill: Some(theme.normal), ..layout },
                listeners: listeners,
                ..,
                { rows }
            }
        }
    };

    let left = column(
        DiffVersion::Original,
        Layout {
            basis: Basis::Length(
                (u32::from(width.saturating_sub(1)) * u32::from(divider) / 100) as u16,
            ),
            min_width: original_width + MIN_TEXT,
            ..Default::default()
        },
    );
    // The right column takes what the left one left, so the two together are
    // the pane however the arithmetic rounded.
    let right = column(
        DiffVersion::Modified,
        Layout { grow: 1, min_width: modified_width + MIN_TEXT, ..Default::default() },
    );

    rsx! {
        Row {
            ref: Some(node),
            layout: Layout {
                grow: 1,
                min_width: original_width + modified_width + 1 + MIN_TEXT * 2,
                ..Default::default()
            },
            ..,
            { left }
            Divider {
                layout: Layout { basis: Basis::Length(1), shrink: 0, ..Default::default() },
                symbol: "│",
                style: theme.normal.patch(theme.divider),
                ..
            }
            { right }
        }
    }
}
