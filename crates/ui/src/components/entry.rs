//! One row of the file list: indent, body, status.

use std::rc::Rc;

use loom::{
    Basis, Canvas, CanvasProps, Element, Layout, Node, Row, RowProps, Scope, component, rsx,
    use_context,
};
use ratatui::style::Style;

use super::context::ThemeContext;
use crate::cells;

/// The tree lines to the left of the name. Fixed by depth.
#[derive(Clone, PartialEq, Eq)]
pub struct Indent {
    pub lines: Rc<str>,
}

/// The icon and the name. Absorbs whatever room is left, and truncates.
#[derive(Clone, PartialEq, Eq)]
pub struct Body {
    pub icon: &'static str,
    pub text: Rc<str>,
}

/// The counts and the change letter, right-aligned.
#[derive(Clone, PartialEq, Eq)]
pub struct Status {
    pub added: u32,
    pub removed: u32,
    pub letter: &'static str,
}

/// What a row of the explorer is.
#[derive(Clone, PartialEq, Eq)]
pub enum Content {
    Heading { name: Rc<str>, files: usize },
    Directory { name: Rc<str>, open: bool },
    File { name: Rc<str> },
}

/// One row: the indent takes its width, the status takes its width, and the
/// body gets the rest.
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

    let indent_width = indent.lines.chars().count() as u16;
    let counts = status.as_ref().map(|status| {
        let mut text = String::new();
        if status.added > 0 {
            text.push_str(&format!("+{} ", status.added));
        }
        if status.removed > 0 {
            text.push_str(&format!("-{} ", status.removed));
        }
        text.push_str(status.letter);
        text
    });
    let status_width = counts.as_ref().map_or(0, |text| text.chars().count() as u16 + 1);

    let lines = Rc::clone(&indent.lines);
    let label: Rc<str> = Rc::from(format!("{} {}", body.icon, body.text).as_str());
    let counts: Rc<str> = Rc::from(counts.unwrap_or_default().as_str());

    rsx! {
        Row {
            layout: Layout { basis: Basis::Length(1), shrink: 0, fill: Some(base), ..Default::default() },
            ..,
            { painted(lines, base, 0, Layout { basis: Basis::Length(indent_width), shrink: 0, ..Default::default() }) }
            { truncating(label, base, Layout { grow: 1, ..Default::default() }) }
            { painted(counts, base, 0, Layout { basis: Basis::Length(status_width), shrink: 0, ..Default::default() }) }
        }
    }
}

/// A section that writes what it is given and stops at its own edge.
fn painted(text: Rc<str>, style: Style, offset: u16, layout: Layout) -> Node {
    Canvas::build(
        CanvasProps {
            layout,
            paint: Rc::new(move |brush: &mut loom::Paint<'_>| {
                let area = brush.area();
                cells::fill(brush.cells(), area, style);
                cells::write(brush.cells(), area, offset, &text, style);
            }),
            ..Default::default()
        },
        None,
    )
}

/// The body, cut with an ellipsis when it does not fit.
fn truncating(text: Rc<str>, style: Style, layout: Layout) -> Node {
    Canvas::build(
        CanvasProps {
            layout,
            paint: Rc::new(move |brush: &mut loom::Paint<'_>| {
                let area = brush.area();
                cells::fill(brush.cells(), area, style);
                let room = area.width as usize;
                let length = text.chars().count();
                if length <= room {
                    cells::write(brush.cells(), area, 0, &text, style);
                } else if room > 1 {
                    let kept: String = text.chars().take(room - 1).collect();
                    cells::write(brush.cells(), area, 0, &format!("{kept}…"), style);
                }
            }),
            ..Default::default()
        },
        None,
    )
}
