//! The bottom row: what a reviewer needs to know without asking.
//!
//! One row, because every row it takes is a row of diff it hides.

use std::rc::Rc;

use loom::{
    Basis, Canvas, CanvasProps, Layout, Node, Row, RowProps, Scope, component, rsx, use_context,
    use_memo, use_sync_external_store,
};
use ratatui::style::Style;

use super::context::{
    CursorContext, DiffStoreContext, ExhaustedContext, FileContext, LayoutContext, NoticeContext,
    ThemeContext, ViewLinesContext,
};
use crate::cells;

/// The left section: what is being shown. Truncates when narrow.
///
/// Several runs, because a name and the note beside it are not the same
/// thing and must not look like one.
pub struct Title {
    pub runs: Vec<(Rc<str>, Style)>,
}

impl Title {
    fn one(text: impl AsRef<str>, style: Style) -> Self {
        Self { runs: vec![(Rc::from(text.as_ref()), style)] }
    }

    fn width(&self) -> u16 {
        self.runs.iter().map(|(text, _)| text.chars().count() as u16).sum()
    }
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
    let view_lines = use_context::<ViewLinesContext>(scope);
    let cursor = use_context::<CursorContext>(scope);
    let notice = use_context::<NoticeContext>(scope);
    let layout = use_context::<LayoutContext>(scope);
    let exhausted = use_context::<ExhaustedContext>(scope);
    let store = use_context::<DiffStoreContext>(scope);
    // The diff worker fills the store; the row subscribes rather than being
    // handed what it produced.
    let reading = use_sync_external_store(scope, &store);

    let base = theme.status;
    let total = view_lines.end;

    let alignment = reading.content.as_ref().and_then(|c| c.alignment());
    // A walk of every view line, so it is done once per diff rather than once
    // per frame.
    let blocks = use_memo(scope, (reading.clone(), layout), || {
        alignment.map(|alignment| alignment.blocks(layout)).unwrap_or_default()
    });

    // `file` is the focused pane's, so it is `None` exactly when the reader is
    // in the list — which has no changes to count and no engine that could
    // have given up on it.
    let has_file = file.is_some();
    let changes = if has_file { blocks.len() } else { 0 };
    let change = has_file
        .then(|| blocks.iter().position(|block| block.contains(&cursor)))
        .flatten();
    let timed_out = has_file && alignment.is_some_and(|alignment| alignment.hit_timeout());

    let sidecar = Sidecar {
        text: Rc::from(
            summary(has_file, cursor, total.max(1), changes, change, exhausted).as_str(),
        ),
        style: base,
    };
    let title = match (notice, file) {
        (Some(why), _) => Title { runs: vec![(why, base.patch(theme.warning))] },
        (None, Some(file)) => name_of(&file, base, &theme),
        (None, None) => Title::one("changed files", base.patch(theme.status_path)),
    };

    // Loud. A diff the engine abandoned is not a diff, and a reviewer who
    // mistakes one for a complete one will approve code they have not seen.
    let mut title = title;
    if timed_out {
        title.runs.push((
            Rc::from("  PARTIAL — diff timed out"),
            base.patch(theme.warning),
        ));
    }
    let _ = title.width();

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
            { runs_canvas(title.runs, base, 1, Layout { grow: 1, ..Default::default() }) }
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

/// The path, and the note beside it in its own style.
fn name_of(file: &file_types::File, base: Style, theme: &crate::theme::Theme) -> Title {
    let path = file.path();
    let styled = base.patch(theme.status_path);
    let mut runs = Vec::new();

    let directory = path.directory();
    if !directory.is_empty() {
        runs.push((Rc::from(directory), base.patch(theme.divider)));
        runs.push((Rc::from("/"), base.patch(theme.divider)));
    }
    runs.push((Rc::from(path.file_name()), styled));

    let note = match file.is_one_sided() {
        Some(file_types::DiffVersion::Modified) => "   (added)",
        Some(file_types::DiffVersion::Original) => "   (deleted)",
        None => "",
    };
    if !note.is_empty() {
        runs.push((Rc::from(note), base));
    }

    Title { runs }
}

/// Several runs, written from `offset` and cut at the section's edge.
fn runs_canvas(runs: Vec<(Rc<str>, Style)>, base: Style, offset: u16, layout: Layout) -> Node {
    use loom::Element;
    Canvas::build(
        CanvasProps {
            layout,
            paint: Rc::new(move |paint: &mut loom::Paint<'_>| {
                let area = paint.area();
                cells::fill(paint.cells(), area, base);
                let mut at = offset;
                for (text, style) in &runs {
                    at = cells::write(paint.cells(), area, at, text, *style);
                }
            }),
            ..Default::default()
        },
        None,
    )
}

/// A list of changed files is not a diff, so it has no changes to count.
fn summary(
    has_file: bool,
    cursor: u32,
    total: u32,
    changes: usize,
    change: Option<usize>,
    exhausted: Option<crate::components::Direction>,
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
            crate::components::Direction::Next => "next",
            crate::components::Direction::Previous => "previous",
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



