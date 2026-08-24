//! One file, no comparison: one gutter and one text column.

use std::rc::Rc;

use align::DiffVersion;
use loom::{
    use_layout_effect, use_ref,
    Basis, Bubble, Column, ColumnProps, Layout, Listeners, Mouse, Node, Row, RowProps, Scope,
    capture_pointer, component, release_pointer, rsx, use_context, use_state,
};
use ratatui::style::Style;

use super::context::{
    ColoursContext, CursorContext, FirstCellContext, PaneContext, ScreenMapContext, SyntaxOnContext,
    ThemeContext, ViewLinesContext,
};
use super::{CodeText, CodeTextProps, Gutter, GutterProps, clip_to_line, gutter_width};
use crate::state::selection::{Pos, Selection, SelectionColumn};

/// How narrow the text column may get before the pane refuses to draw.
const MIN_TEXT: u16 = 4;

/// One file on its own: a gutter and the text.
///
/// No `DiffVersion` and no row kind, so two branches do the whole of it —
/// the cursor line, or a plain one.
#[component]
pub fn SingleFile(
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
    let first_cell = use_context::<FirstCellContext>(scope);


    let store = colours.borrow();
    let read = view.borrow();
    let selection = read
        .selection
        .filter(|(owner, _)| Some(*owner) == pane)
        .map(|(_, held)| held);
    let held = read.buffer(*buffer);
    let Some(alignment) = held.alignment() else { return Node::Empty };
    let Some(file) = held.file() else { return Node::Empty };
    let lines = alignment.lines(DiffVersion::Modified).len() as u32;
    let width = gutter_width(lines);

    let spans = if syntax_on {
        crate::state::buffer::colour::spans_for(file, &store)
    } else {
        syntax::Spans::Off
    };


    // The text column is the row less its gutter.
    let node = use_ref(scope, || None::<loom::NodeHandle>);
    let filling = Rc::clone(&map);
    use_layout_effect(scope, loom::Always, move || {
        let Some(pane) = pane else { return };
        let Some(node) = *node.current() else { return };
        let area = node.area();
        filling.borrow_mut().text_areas.push(crate::screen_map::TextArea {
            pane,
            column: SelectionColumn::Only,
            rect: ratatui::layout::Rect {
                x: area.x.saturating_add(width),
                width: area.width.saturating_sub(width),
                ..area
            },
        });
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
                    Gutter {
                        number: Some(number),
                        style: numbers,
                        blank: base,
                        width: width,
                    }
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
            layout: Layout { grow: 1, min_width: width + MIN_TEXT, ..Default::default() },
            ..,
            { rows }
        }
    }
}
