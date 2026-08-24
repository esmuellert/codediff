//! One file, no comparison: one gutter and one text column.

use std::rc::Rc;

use align::DiffVersion;
use loom::{
    Basis, Bubble, Column, ColumnProps, Layout, Listeners, Mouse, Node, Row, RowProps, Scope,
    capture_pointer, component, release_pointer, rsx, use_context, use_layout_effect, use_ref,
    use_sync_external_store,
};
use ratatui::style::Style;

use super::context::{
    CursorContext, DiffStoreContext, FirstCellContext, OnSelectContext, PaneContext,
    ScreenMapCellContext, SelectionContext, SyntaxOnContext, ThemeContext, ViewLinesContext,
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
/// The selection is held above this component: a new file ends one, and that
/// is not this component's to know.
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
    let selection = use_context::<SelectionContext>(scope);
    let on_select = use_context::<OnSelectContext>(scope);
    let pane = use_context::<PaneContext>(scope);
    let map = use_context::<ScreenMapCellContext>(scope);
    // The workers fill the store; this subscribes rather than being handed
    // what they produced.
    let reading = use_sync_external_store(scope, &diffs);

    // Where a press landed, kept until a drag makes a selection of it. A
    // click that never drags selects nothing, so this is not a selection.
    let pending = use_ref(scope, || None::<Pos>);
    let node = use_ref(scope, || None::<loom::NodeHandle>);
    // How wide the gutter turned out, written by the body below. The effect
    // is declared here, before the early return, because a hook may not run
    // behind a condition.
    let gutter_cells = use_ref(scope, || 0u16);

    let filling = Rc::clone(&map);
    use_layout_effect(scope, loom::Always, move || {
        let Some(pane) = pane else { return };
        let Some(node) = *node.current() else { return };
        let gutter = *gutter_cells.current();
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
                x: area.x.saturating_add(gutter),
                width: area.width.saturating_sub(gutter),
                ..area
            },
        });
    });

    let Some(content) = reading.content.as_ref() else {
        return Node::Empty;
    };
    let (lines_data, _file) = match content.as_ref() {
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
    // when the reader has turned colour off. The borrow is held for the whole
    // body, because the spans are borrowed from it rather than copied.
    let colours = reading.colours.borrow();
    let spans = if syntax_on {
        match content.as_ref() {
            // A lone file has one side, and it is the side it exists on — an
            // added file has no original to be coloured as.
            pipeline::file::DiffContent::SingleFile(single) => {
                crate::state::buffer::colour::spans_single_file(single, &colours)
            }
            pipeline::file::DiffContent::Diff(diff) => {
                crate::state::buffer::colour::spans_for(&diff.file, &colours)
            }
        }
    } else {
        syntax::Spans::Off
    };

    let lines = line_count;
    let width = gutter_width(lines);
    *gutter_cells.current() = width;

    // Where in the file a pointer landed, or `None` over the gutter.
    let top = view_lines.start;
    let last = view_lines.end.saturating_sub(1);
    let at = move |mouse: Mouse| -> Option<Pos> {
        let x = mouse.local.x.checked_sub(width)?;
        let line = (top + u32::from(mouse.local.y)).min(last);
        Some(Pos::new(line, first_cell + u32::from(x)))
    };

    // The pointer is captured on the way down, so a drag that leaves the
    // column keeps arriving until the button comes up. The press itself is
    // passed on, so that clicking a diff also moves the focus into it.
    let start = Rc::clone(&on_select);
    let drag = Rc::clone(&on_select);
    let end = Rc::clone(&on_select);
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
            ref: Some(node),
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
