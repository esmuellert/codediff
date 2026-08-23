//! One file, no comparison: one gutter and one text column.

use std::rc::Rc;

use align::DiffVersion;
use line_index::DEFAULT_TAB_WIDTH;
use loom::{
    Basis, Bubble, Column, ColumnProps, Layout, Listeners, Mouse, Node, Row, RowProps, Scope,
    capture_pointer, component, release_pointer, rsx, use_context, use_state,
};
use ratatui::style::Style;

use super::context::{
    CursorContext, DiffsContext, FirstCellContext, ThemeContext, ViewLinesContext,
};
use super::{CodeText, CodeTextProps, Gutter, GutterProps, clip_to_line, gutter_width};
use crate::view::selection::{Pos, Selection, SelectionColumn};

/// How narrow the text column may get before the pane refuses to draw.
const MIN_TEXT: u16 = 4;

/// One file on its own: a gutter and the text.
///
/// No `DiffVersion` and no row kind, so two branches do the whole of it —
/// the cursor line, or a plain one.
#[component]
pub fn SingleFile(scope: &mut Scope) -> Node {
    let theme = use_context::<ThemeContext>(scope);
    let diffs = use_context::<DiffsContext>(scope);
    let view_lines = use_context::<ViewLinesContext>(scope);
    let cursor = use_context::<CursorContext>(scope);
    let first_cell = use_context::<FirstCellContext>(scope);

    let reading = loom::use_sync_external_store(scope, &diffs);
    let (selection, set_selection) = use_state(scope, || None::<Selection>);

    let Some(file) = reading.diff.clone() else { return Node::Empty };
    let alignment = &file.alignment;
    let lines = alignment.lines(DiffVersion::Modified).len() as u32;
    let width = gutter_width(lines);

    let spans = if reading.syntax_on {
        crate::view::buffer::colour::spans_diff(&file, &reading.colours)
    } else {
        syntax::Spans::Off
    };

    let top = view_lines.start;
    let at = move |mouse: Mouse| -> Option<Pos> {
        let x = mouse.local.x.checked_sub(width)?;
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

    let rows: Vec<Node> = view_lines
        .clone()
        .take_while(|line| *line < lines)
        .map(|line| {
            let is_cursor = line == cursor;
            let base = theme
                .normal
                .patch(if is_cursor { theme.cursor_line } else { Style::new() });
            let numbers = base.patch(if is_cursor {
                theme.line_number_current
            } else {
                theme.line_number
            });

            // The gutter shows `line + 1`, and so does the syntax lookup.
            let number = line + 1;
            let text: Rc<str> = Rc::from(alignment.line(DiffVersion::Modified, number).unwrap_or(""));
            let syntax: Rc<[syntax::Span]> = Rc::from(spans.line(DiffVersion::Modified, number));

            rsx! {
                Row {
                    key: line,
                    layout: Layout { basis: Basis::Length(1), shrink: 0, ..Default::default() },
                    ..,
                    Gutter { number: number, width: width, style: numbers }
                    CodeText {
                        text: text,
                        diff: Rc::from(&[][..]),
                        syntax: syntax,
                        code: Rc::new(theme.code.clone()),
                        unchanged_style: base,
                        changed_style: base,
                        selection: clip_to_line(selection.as_ref(), line),
                        first_cell: first_cell,
                        selected_style: theme.selection,
                    }
                }
            }
        })
        .collect();

    let _ = DEFAULT_TAB_WIDTH;

    rsx! {
        Column {
            layout: Layout { grow: 1, min_width: width + MIN_TEXT, ..Default::default() },
            listeners: listeners,
            ..,
            { rows }
        }
    }
}
