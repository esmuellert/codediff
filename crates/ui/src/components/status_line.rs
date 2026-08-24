//! The bottom row: what a reviewer needs to know without asking.
//!
//! One row, because every row it takes is a row of diff it hides.

use std::rc::Rc;

use loom::{
    Basis, Canvas, CanvasProps, Layout, Node, Row, RowProps, Scope, component, rsx, use_context,
};
use ratatui::style::Style;

use super::context::{CursorContext, FileContext, NoticeContext, ThemeContext, ViewLinesContext};
use crate::cells;

/// The left section: what is being shown. Truncates when narrow.
pub struct Title {
    pub text: Rc<str>,
    pub style: Style,
}

/// The right section: where in it the cursor is. Keeps its width.
pub struct Sidecar {
    pub text: Rc<str>,
    pub style: Style,
}

/// The bottom row.
#[component]
pub fn StatusLine(
    scope: &mut Scope,
    changes: usize,
    change: Option<usize>,
    timed_out: bool,
    exhausted: Option<crate::state::Direction>,
) -> Node {
    let theme = use_context::<ThemeContext>(scope);
    let file = use_context::<FileContext>(scope);
    let view_lines = use_context::<ViewLinesContext>(scope);
    let cursor = use_context::<CursorContext>(scope);
    let notice = use_context::<NoticeContext>(scope);

    let base = theme.status;
    let total = view_lines.end;
    let (changes, change) = (*changes, *change);

    let sidecar = Sidecar {
        text: Rc::from(
            summary(file.is_some(), cursor, total.max(1), changes, change, *exhausted).as_str(),
        ),
        style: base,
    };
    let title = match (notice, file) {
        (Some(why), _) => Title { text: why, style: base.patch(theme.warning) },
        (None, Some(file)) => Title {
            text: Rc::from(path_of(&file).as_str()),
            style: base.patch(theme.status_path),
        },
        (None, None) => Title {
            text: Rc::from("changed files"),
            style: base.patch(theme.status_path),
        },
    };

    // Loud. A diff the engine abandoned is not a diff, and a reviewer who
    // mistakes one for a complete one will approve code they have not seen.
    let title = if *timed_out {
        Title {
            text: Rc::from(format!("{}  PARTIAL — diff timed out", title.text).as_str()),
            style: base.patch(theme.warning),
        }
    } else {
        title
    };

    let sidecar_width = sidecar.text.chars().count() as u16 + 1;

    rsx! {
        Row {
            layout: Layout {
                basis: Basis::Length(1),
                shrink: 0,
                fill: Some(base),
                ..Default::default()
            },
            ..,
            { text_canvas(title.text, title.style, 1, Layout { grow: 1, ..Default::default() }) }
            {
                text_canvas(
                    sidecar.text,
                    sidecar.style,
                    0,
                    Layout {
                        basis: Basis::Length(sidecar_width),
                        shrink: 0,
                        ..Default::default()
                    },
                )
            }
        }
    }
}

/// One section of the row, written from `offset` and cut at its own edge.
fn text_canvas(text: Rc<str>, style: Style, offset: u16, layout: Layout) -> Node {
    use loom::Element;
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

/// The path, with the directory dropped before the file name is.
fn path_of(file: &file_types::File) -> String {
    let path = file.path();
    let note = match file.is_one_sided() {
        Some(file_types::DiffVersion::Modified) => "   (added)",
        Some(file_types::DiffVersion::Original) => "   (deleted)",
        None => "",
    };
    let directory = path.directory();
    if directory.is_empty() {
        format!("{}{note}", path.file_name())
    } else {
        format!("{directory}/{}{note}", path.file_name())
    }
}

/// A list of changed files is not a diff, so it has no changes to count.
fn summary(
    has_file: bool,
    cursor: u32,
    total: u32,
    changes: usize,
    change: Option<usize>,
    exhausted: Option<crate::state::Direction>,
) -> String {
    let position = format!("{}/{}", cursor + 1, total);
    // A list of changed files is not a diff, so it has no changes to count.
    if !has_file {
        return position;
    }
    // A key that had nowhere to go answers the key that was just pressed; the
    // counter would only repeat what it already said.
    if let Some(direction) = exhausted {
        let which = match direction {
            crate::state::Direction::Next => "next",
            crate::state::Direction::Previous => "previous",
        };
        return format!("no {which} change   {position}");
    }
    match (change, changes) {
        (_, 0) => format!("no changes   {position}"),
        (Some(i), n) => format!("change {}/{n}   {position}", i + 1),
        (None, 1) => format!("1 change   {position}"),
        (None, n) => format!("{n} changes   {position}"),
    }
}



