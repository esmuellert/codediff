//! Two columns of one diff, and the divider between them.

use std::rc::Rc;

use align::{Alignment, DiffVersion, Slot};
use file_types::DiffType;
use loom::{
    Basis, Bubble, Column, ColumnProps, Divider, DividerProps, Element, Key, Layout, Listeners,
    Mouse, Node, Row, RowProps, Scope, capture_pointer, component, release_pointer, rsx,
    use_context, use_layout_effect, use_ref, use_state,
};

use super::context::{
    CursorContext, DiffDataContext, FirstCellContext, ThemeContext, ViewLinesContext,
};
use super::{
    CodeText, CodeTextProps, Filler, FillerProps, Gutter, GutterProps, clip_to_line, gutter_width,
    row_styles,
};
use crate::theme::Theme;
use crate::view::selection::{Pos, Selection, SelectionColumn};

/// How narrow a text column may get before the pane refuses to draw.
const MIN_TEXT: u16 = 4;

/// What one column needs to draw one row.
struct Rows<'a> {
    alignment: &'a Alignment,
    theme: &'a Theme,
    spans: syntax::Spans<'a>,
    cursor: u32,
    top: u32,
    width: u16,
    selection: Option<Selection>,
}

impl Rows<'_> {
    /// One view line, as either a row of text or a band of hatching.
    fn row(&self, offset: usize, line: &align::ViewLine, diff_version: DiffVersion) -> Node {
        let index = self.top + offset as u32;
        let slot = match diff_version {
            DiffVersion::Original => line.original,
            DiffVersion::Modified => line.modified,
        };

        let Slot::Line(number) = slot else {
            // No line on this side at all, so the whole width is hatched and
            // no line number is implied.
            return Filler::build(FillerProps {}, Some(Key::from(offset)));
        };

        let is_cursor = index == self.cursor;
        let (unchanged, changed, numbers) = row_styles(
            self.theme,
            line.kind,
            diff_version,
            self.alignment.moved(diff_version, number).is_some(),
            is_cursor,
        );

        let diff: Rc<[std::ops::Range<u32>]> = self
            .alignment
            .spans(diff_version, number)
            .into_iter()
            .map(|s| s.bytes)
            .collect();

        let syntax: Rc<[syntax::Span]> = Rc::from(self.spans.line(diff_version, number));

        let text: Rc<str> = Rc::from(self.alignment.line(diff_version, number).unwrap_or(""));

        rsx! {
            Row {
                key: offset,
                layout: Layout { basis: Basis::Length(1), shrink: 0, ..Default::default() },
                ..,
                Gutter {
                    number: Some(number),
                    style: numbers,
                    blank: unchanged,
                    width: self.width,
                }
                CodeText {
                    text: text,
                    diff: diff,
                    syntax: syntax,
                    unchanged_style: unchanged,
                    changed_style: changed,
                    selection: clip_to_line(self.selection.as_ref(), index),
                }
            }
        }
    }
}

/// Two columns of one file, side by side, with a divider between them.
///
/// The divider's position and the text selection are this component's own
/// state; no parent needs either.
#[component]
pub fn SideBySide(scope: &mut Scope) -> Node {
    let theme = use_context::<ThemeContext>(scope);
    let diff_data = use_context::<DiffDataContext>(scope);
    let view_lines = use_context::<ViewLinesContext>(scope);
    let cursor = use_context::<CursorContext>(scope);
    let first_cell = use_context::<FirstCellContext>(scope);

    // Percent of the pane the left column takes.
    // The workers fill this; a component subscribes rather than being handed
    // what they produced.
    let loaded = loom::use_sync_external_store(scope, &diff_data);

    let (divider, _set_divider) = use_state(scope, || 50u16);
    let (selection, set_selection) = use_state(scope, || None::<Selection>);

    // The divider is taken off the top before dividing, so widening the pane
    // by one column widens a column rather than the divider. That needs this
    // component's own width, which layout knows and the render body does not.
    let node = use_ref(scope, || None::<loom::NodeHandle>);
    let (width, set_width) = use_state(scope, || 0u16);
    use_layout_effect(scope, loom::Always, move || {
        let now = node.current().map_or(0, |node| node.area().width);
        set_width(&move |_| now);
    });

    let Some(alignment) = loaded.diff.clone() else { return Node::Empty };

    // Collected once and read by both columns, so the two cannot disagree
    // about what line they are on.
    let lines: Vec<align::ViewLine> = alignment
        .alignment
        .view_lines_from(DiffType::SideBySide, view_lines.start)
        .take(view_lines.len())
        .collect();

    let column = |diff_version: DiffVersion, layout: Layout| -> Node {
        let width = gutter_width(alignment.alignment.lines(diff_version).len() as u32);
        let which = match diff_version {
            DiffVersion::Original => SelectionColumn::Original,
            DiffVersion::Modified => SelectionColumn::Modified,
        };

        // Where in the file a pointer landed, or `None` over the gutter.
        let top = view_lines.start;
        let at = move |mouse: Mouse| -> Option<Pos> {
            let x = mouse.local.x.checked_sub(width)?;
            Some(Pos::new(top + u32::from(mouse.local.y), first_cell + u32::from(x)))
        };

        let listeners = Listeners::new()
            .on_mouse_down(move |mouse| {
                if let Some(pos) = at(mouse) {
                    capture_pointer();
                    set_selection(&move |_| Some(Selection::start(which, pos)));
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

        let rows = Rows {
            alignment: &alignment.alignment,
            theme: &theme,
            spans: if loaded.syntax_on {
                crate::view::buffer::colour::spans_diff(&alignment, &loaded.colours)
            } else {
                syntax::Spans::Off
            },
            cursor,
            top: view_lines.start,
            width,
            selection: selection.filter(|held| held.column == which),
        };

        let children: Vec<Node> = lines
            .iter()
            .enumerate()
            .map(|(offset, line)| rows.row(offset, line, diff_version))
            .collect();

        rsx! {
            Column {
                layout: layout,
                listeners: listeners,
                ..,
                { children }
            }
        }
    };

    let original_width = gutter_width(alignment.alignment.lines(DiffVersion::Original).len() as u32);
    let modified_width = gutter_width(alignment.alignment.lines(DiffVersion::Modified).len() as u32);

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
