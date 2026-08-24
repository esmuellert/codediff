//! One file, no comparison: one gutter and one text column.

use std::rc::Rc;

use align::DiffVersion;
use loom::{
    Basis, Bubble, Column, ColumnProps, Layout, Listeners, Mouse, Node, Row, RowProps, Scope,
    capture_pointer, component, release_pointer, rsx, use_context, use_state,
    use_sync_external_store,
};
use ratatui::style::Style;

use super::context::{
    CursorContext, DiffStoreContext, FirstCellContext, SyntaxOnContext, ThemeContext,
    ViewLinesContext,
};
use super::{CodeText, CodeTextProps, Gutter, GutterProps, clip_to_line, gutter_width};
use crate::state::selection::{Pos, Selection, SelectionColumn};

/// How narrow the text column may get before the pane refuses to draw.
const MIN_TEXT: u16 = 4;

/// One file on its own: a gutter and the text.
///
/// No row kind and one version, so two branches do the whole of it — the
/// cursor line, or a plain one. The lines come from the modified side, which
/// is the side a lone file is read as.
///
/// The text selection is this component's own state; no parent needs it.
enum LinesSource<'a> {
    Alignment(&'a align::Alignment, u32),
    Plain(&'a [String]),
}

#[component]
pub fn SingleFile(scope: &mut Scope) -> Node {
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
    let (lines_data, file) = match content.as_ref() {
        pipeline::file::DiffContent::Diff(diff) => {
            let n = diff.alignment.lines(DiffVersion::Modified).len() as u32;
            (LinesSource::Alignment(&diff.alignment, n), &diff.file)
        }
        pipeline::file::DiffContent::SingleFile(single) => {
            (LinesSource::Plain(&single.lines), &single.file)
        }
    };
    let line_count = match &lines_data {
        LinesSource::Alignment(_, n) => *n,
        LinesSource::Plain(lines) => lines.len() as u32,
    };

    // How the syntax worker has coloured the file so far, or nothing at all
    // when the reader has turned colour off.
    let spans = if syntax_on {
        match &lines_data {
        LinesSource::Alignment(_, _) => crate::state::buffer::colour::spans_for(file, &reading.colours),
        LinesSource::Plain(_) => syntax::Spans::Off,
    }
    } else {
        syntax::Spans::Off
    };

    let lines = line_count;
    let width = gutter_width(lines);

    // Where in the file a pointer landed, or `None` over the gutter.
    let top = view_lines.start;
    let at = move |mouse: Mouse| -> Option<Pos> {
        let x = mouse.local.x.checked_sub(width)?;
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
            let syntax: Rc<[syntax::Span]> = Rc::from(spans.line(DiffVersion::Modified, number));
            let text: Rc<str> = Rc::from(match &lines_data {
                LinesSource::Alignment(a, _) => a.line(DiffVersion::Modified, number).unwrap_or(""),
                LinesSource::Plain(v) => v.get(line as usize).map(|s| s.as_str()).unwrap_or(""),
            });

            rsx! {
                Row {
                    key: line,
                    layout: Layout { basis: Basis::Length(1), shrink: 0, ..Default::default() },
                    ..,
                    Gutter {
                        number: Some(number),
                        style: numbers,
                        blank: base,
                        width: width,
                    }
                    // Nothing differs from anything, so there are no diff
                    // spans and one style does both roles.
                    CodeText {
                        text: text,
                        diff: Rc::from(&[][..]),
                        syntax: syntax,
                        unchanged_style: base,
                        changed_style: base,
                        selection: clip_to_line(selection.as_ref(), line),
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
                min_width: width + MIN_TEXT,
                fill: Some(theme.normal),
                ..Default::default()
            },
            listeners: listeners,
            ..,
            { rows }
        }
    }
}
