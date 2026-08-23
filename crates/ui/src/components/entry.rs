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

/// The tree lines to the left of the name. Fixed by depth.
#[derive(Clone, PartialEq, Eq)]
pub struct Indent {
    /// `"│ └ "` or `""`.
    pub lines: Rc<str>,
}

/// The icon and the name. Absorbs whatever room is left, and truncates.
#[derive(Clone, PartialEq, Eq)]
pub struct Body {
    pub icon: Icon,
    pub text: Rc<str>,
}

/// The counts and the change letter, right-aligned.
#[derive(Clone, PartialEq, Eq)]
pub struct Status {
    pub added: u32,
    pub removed: u32,
    pub letter: &'static str,
}

impl Status {
    /// `+4 -3 M` when both sides changed, `+4 M` when only one. The zero is
    /// omitted so it does not repeat down a column the eye is scanning.
    fn text(&self) -> String {
        let mut out = String::new();
        if self.added > 0 {
            out.push_str(&format!("+{} ", self.added));
        }
        if self.removed > 0 {
            out.push_str(&format!("-{} ", self.removed));
        }
        out.push_str(self.letter);
        out
    }
}

/// What a row of the explorer is.
#[derive(Clone, PartialEq, Eq)]
pub enum Content {
    Heading { name: Rc<str>, files: usize, stats: Stats },
    Directory { name: Rc<str>, open: bool, depth: u16 },
    File { name: Rc<str>, file: Rc<file_types::File> },
}

/// What a heading counts up across the files under it.
#[derive(Clone, Copy, PartialEq, Eq, Default)]
pub struct Stats {
    pub added: u32,
    pub removed: u32,
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

    let lines = Rc::clone(&indent.lines);
    let indent_width = lines.chars().count() as u16;

    let full = status.as_ref().map(Status::text).unwrap_or_default();
    let letter = status.as_ref().map_or("", |status| status.letter);
    let status_width = full.chars().count() as u16;
    let letter_width = letter.chars().count() as u16;

    let icon = body.icon;
    let text = Rc::clone(&body.text);
    let icon_style = base.patch(Style::new().fg(icon.color));

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
                written(
                    lines,
                    base,
                    Layout { basis: Basis::Length(indent_width), shrink: 0, ..Default::default() },
                )
            }
            {
                named(
                    icon.glyph,
                    icon_style,
                    text,
                    base,
                    Layout { grow: 1, ..Default::default() },
                )
            }
            {
                counted(
                    full,
                    letter.to_string(),
                    base,
                    Layout {
                        basis: Basis::Length(status_width),
                        // When the counts do not fit, the letter still does.
                        min_width: letter_width,
                        shrink: 1,
                        ..Default::default()
                    },
                )
            }
        }
    }
}

/// A section that writes what it is given and stops at its own edge.
fn written(text: Rc<str>, style: Style, layout: Layout) -> Node {
    Canvas::build(
        CanvasProps {
            layout,
            paint: Rc::new(move |paint: &mut loom::Paint<'_>| {
                let area = paint.area();
                cells::fill(paint.cells(), area, style);
                cells::write(paint.cells(), area, 0, &text, style);
            }),
            ..Default::default()
        },
        None,
    )
}

/// The icon in its own colour, then the name, cut with `…` when it does not
/// fit.
fn named(glyph: char, icon_style: Style, text: Rc<str>, base: Style, layout: Layout) -> Node {
    Canvas::build(
        CanvasProps {
            layout,
            paint: Rc::new(move |paint: &mut loom::Paint<'_>| {
                let area = paint.area();
                cells::fill(paint.cells(), area, base);
                let mut at = cells::write(paint.cells(), area, 0, &glyph.to_string(), icon_style);
                at += 1;

                let room = area.width.saturating_sub(at) as usize;
                let length = text.chars().count();
                if length <= room {
                    cells::write(paint.cells(), area, at, &text, base);
                } else if room > 1 {
                    let kept: String = text.chars().take(room - 1).collect();
                    cells::write(paint.cells(), area, at, &format!("{kept}…"), base);
                }
            }),
            ..Default::default()
        },
        None,
    )
}

/// The counts and the letter, right-aligned. When the counts do not fit, the
/// letter is what is left.
fn counted(full: String, letter: String, style: Style, layout: Layout) -> Node {
    Canvas::build(
        CanvasProps {
            layout,
            paint: Rc::new(move |paint: &mut loom::Paint<'_>| {
                let area = paint.area();
                cells::fill(paint.cells(), area, style);
                let shown = if full.chars().count() as u16 <= area.width { &full } else { &letter };
                let at = area.width.saturating_sub(shown.chars().count() as u16);
                cells::write(paint.cells(), area, at, shown, style);
            }),
            ..Default::default()
        },
        None,
    )
}
