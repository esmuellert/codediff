//! The bottom row: what a reviewer needs to know without asking.
//!
//! One row, because every row it takes is a row of diff it hides.

use std::rc::Rc;

use loom::{
    Basis, Canvas, CanvasProps, Layout, Node, Row, RowProps, Scope, component, rsx, use_context,
};
use ratatui::style::Style;

use super::context::{CursorContext, DiffDataContext, FileContext, NoticeContext, ThemeContext};
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
pub fn StatusLine(scope: &mut Scope) -> Node {
    let theme = use_context::<ThemeContext>(scope);
    let file = use_context::<FileContext>(scope);
    let cursor = use_context::<CursorContext>(scope);
    let notice = use_context::<NoticeContext>(scope);
    let diff_data = use_context::<DiffDataContext>(scope);

    let loaded = loom::use_sync_external_store(scope, &diff_data);
    let base = theme.status;

    let total = loaded
        .diff
        .as_ref()
        .map_or(0, |diff| diff.alignment.view_lines(file_types::DiffType::SideBySide).count() as u32);
    let changes = loaded.diff.as_ref().map_or(0, |diff| runs(&diff.alignment));
    let change = loaded
        .diff
        .as_ref()
        .and_then(|diff| run_at(&diff.alignment, cursor));

    let sidecar = Sidecar {
        text: Rc::from(summary(file.is_some(), cursor, total.max(1), changes, change).as_str()),
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
fn summary(has_file: bool, cursor: u32, total: u32, changes: usize, change: Option<usize>) -> String {
    let position = format!("{}/{}", cursor + 1, total);
    if !has_file {
        return position;
    }
    match (change, changes) {
        (_, 0) => format!("no changes   {position}"),
        (Some(i), n) => format!("change {}/{n}   {position}", i + 1),
        (None, 1) => format!("1 change   {position}"),
        (None, n) => format!("{n} changes   {position}"),
    }
}

/// How many runs of changed view lines the file has.
fn runs(alignment: &align::Alignment) -> usize {
    let mut count = 0;
    let mut inside = false;
    for line in alignment.view_lines_from(file_types::DiffType::SideBySide, 0) {
        let changed = line.kind != align::ViewLineType::Unchanged;
        if changed && !inside {
            count += 1;
        }
        inside = changed;
    }
    count
}

/// Which run the cursor is in, if any.
fn run_at(alignment: &align::Alignment, cursor: u32) -> Option<usize> {
    let mut count = 0;
    let mut inside = false;
    for (index, line) in alignment
        .view_lines_from(file_types::DiffType::SideBySide, 0)
        .enumerate()
    {
        let changed = line.kind != align::ViewLineType::Unchanged;
        if changed && !inside {
            count += 1;
        }
        inside = changed;
        if index as u32 == cursor {
            return changed.then(|| count - 1);
        }
    }
    None
}

