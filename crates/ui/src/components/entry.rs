//! One row of the file list: indent, body, status.

use std::rc::Rc;

use loom::{
    Basis, Canvas, CanvasProps, Element, Layout, Node, Row, RowProps, Scope, component, rsx,
    use_context,
};
use ratatui::style::Style;

use super::context::ThemeContext;
use crate::cells;
use crate::theme::icon::Icon;

/// One stretch of a row in one colour.
///
/// A row is not one colour: a heading's name and its count differ, and so do
/// the two halves of `+4 -3`.
#[derive(Clone, PartialEq, Eq)]
pub struct Run {
    pub text: Rc<str>,
    pub style: Style,
}

impl Run {
    pub fn new(text: impl AsRef<str>, style: Style) -> Self {
        Self { text: Rc::from(text.as_ref()), style }
    }

    fn width(&self) -> u16 {
        line_index::LineIndex::new(&self.text, 1).width().0 as u16
    }
}

/// The tree lines to the left of the name. Fixed by depth.
#[derive(Clone, PartialEq, Eq)]
pub struct Indent {
    /// `"│ └ "` or `""`.
    pub lines: Rc<str>,
    pub style: Style,
}

/// The icon and the name. Absorbs whatever room is left, and truncates.
#[derive(Clone, PartialEq, Eq)]
pub struct Body {
    /// A heading has none: it names a comparison, not a file.
    pub icon: Option<Icon>,
    pub runs: Rc<[Run]>,
}

/// The counts and the change letter, right-aligned.
///
/// The letter is separate because it is what survives when the counts will
/// not fit.
#[derive(Clone, PartialEq, Eq)]
pub struct Status {
    pub counts: Rc<[Run]>,
    pub letter: Run,
}

/// One row.
///
/// Indent is fixed by depth. Status is fixed by content. Body absorbs
/// whatever is left and is the only section that truncates.
#[component]
pub fn Entry(
    scope: &mut Scope,
    indent: Indent,
    body: Body,
    status: Option<Status>,
    selected: bool,
) -> Node {
    let theme = use_context::<ThemeContext>(scope);
    let base = if *selected {
        theme.normal.patch(theme.cursor_line)
    } else {
        theme.normal
    };

    let indent_width = Run::new(&indent.lines, base).width();
    let icon_width = if body.icon.is_some() { 2 } else { 0 };

    let counts_width = status
        .as_ref()
        .map_or(0, |status| status.counts.iter().map(Run::width).sum::<u16>());
    let letter_width = status.as_ref().map_or(0, |status| status.letter.width());
    let status_width = counts_width + letter_width;

    let lines = Run::new(&indent.lines, indent.style);
    let icon = body.icon;
    let runs = Rc::clone(&body.runs);
    let status = status.clone();

    rsx! {
        Row {
            layout: Layout {
                basis: Basis::Length(1),
                shrink: 0,
                fill: Some(base),
                ..Default::default()
            },
            ..,
            {
                one_run(
                    lines,
                    base,
                    Layout { basis: Basis::Length(indent_width), shrink: 0, ..Default::default() },
                )
            }
            {
                icon_and_name(
                    icon,
                    runs,
                    base,
                    Layout {
                        grow: 1,
                        // The icon and one column of the name, so a row never
                        // shrinks to nothing.
                        min_width: icon_width + 1,
                        ..Default::default()
                    },
                )
            }
            {
                counts_and_letter(
                    status,
                    base,
                    Layout {
                        basis: Basis::Length(status_width),
                        // When the counts will not fit, the letter still does.
                        min_width: letter_width,
                        shrink: 1,
                        ..Default::default()
                    },
                )
            }
        }
    }
}

/// One run, written from the left edge and cut at its own.
fn one_run(run: Run, base: Style, layout: Layout) -> Node {
    Canvas::build(
        CanvasProps {
            layout,
            paint: Rc::new(move |paint: &mut loom::Paint<'_>| {
                let area = paint.area();
                cells::fill(paint.cells(), area, base);
                cells::write(paint.cells(), area, 0, &run.text, run.style);
            }),
            ..Default::default()
        },
        None,
    )
}

/// The icon in its own colour, then the runs, cut when they do not fit.
///
/// Cut rather than dropped: a row with no name says nothing at all.
fn icon_and_name(icon: Option<Icon>, runs: Rc<[Run]>, base: Style, layout: Layout) -> Node {
    Canvas::build(
        CanvasProps {
            layout,
            paint: Rc::new(move |paint: &mut loom::Paint<'_>| {
                let area = paint.area();
                cells::fill(paint.cells(), area, base);

                let mut at = 0u16;
                if let Some(icon) = icon {
                    let style = base.fg(icon.color);
                    at = cells::write(paint.cells(), area, at, &format!("{} ", icon.glyph), style);
                }

                for run in runs.iter() {
                    if at >= area.width {
                        break;
                    }
                    let room = area.width - at;
                    // Never through the middle of a character: a wide one that
                    // will not fit is left out, a column short rather than a
                    // broken glyph.
                    let text = cut(&run.text, room);
                    at = cells::write(paint.cells(), area, at, &text, run.style);
                }
            }),
            ..Default::default()
        },
        None,
    )
}

/// The counts and the letter, right-aligned. When the counts will not fit,
/// the letter is what is left.
fn counts_and_letter(status: Option<Status>, base: Style, layout: Layout) -> Node {
    Canvas::build(
        CanvasProps {
            layout,
            paint: Rc::new(move |paint: &mut loom::Paint<'_>| {
                let area = paint.area();
                cells::fill(paint.cells(), area, base);
                let Some(status) = &status else { return };

                let counts: u16 = status.counts.iter().map(Run::width).sum();
                let letter = status.letter.width();
                let shown: &[Run] =
                    if counts + letter <= area.width { &status.counts } else { &[] };

                let width = shown.iter().map(Run::width).sum::<u16>() + letter;
                let mut at = area.width.saturating_sub(width);
                for run in shown {
                    at = cells::write(paint.cells(), area, at, &run.text, run.style);
                }
                cells::write(paint.cells(), area, at, &status.letter.text, status.letter.style);
            }),
            ..Default::default()
        },
        None,
    )
}

/// Cuts `text` to `room` columns, never through a character.
fn cut(text: &str, room: u16) -> String {
    let line = line_index::LineIndex::new(text, 1);
    if line.width().0 as u16 <= room {
        return text.to_string();
    }
    let end = line.cell_to_byte(line_index::CellCol(u32::from(room)));
    text[..end.0 as usize].to_string()
}
