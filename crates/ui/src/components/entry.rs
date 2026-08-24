//! One row of the file list: indent, body, status.

use std::rc::Rc;

use loom::{Basis, Canvas, CanvasProps, Layout, Node, Scope, component, rsx, use_context};
use ratatui::style::Style;

use super::context::ThemeContext;
use crate::cells;
use crate::theme::icon::Icon;

/// The one column that always separates the two sides.
const GAP: u16 = 1;

/// What is dropped first when a row will not fit, lowest first.
///
/// Named rather than written as numbers at each use, so that "counts go
/// before the guides" is a statement in one place instead of a comparison a
/// reader has to make between two distant literals.
pub mod priority {
    /// Where a moved file came from: useful, never essential.
    pub const MOVED: u8 = 0;
    /// The line counts.
    pub const COUNTS: u8 = 1;
    /// How many files a section holds.
    pub const FILES: u8 = 2;
    /// The indent guides. Last to go, because losing them makes the tree
    /// plainer where losing anything else makes it wrong.
    pub const GUIDES: u8 = 3;
}

/// One stretch of a row in one colour, and whether it may go.
///
/// A row is not one colour: a heading's name and its count differ, and so do
/// the two halves of `+4 -3`.
#[derive(Clone, PartialEq, Eq)]
pub struct Run {
    pub text: Rc<str>,
    pub style: Style,
    /// `None` is never dropped. A file's name has none, because a row with no
    /// name says nothing at all — it is cut with an ellipsis instead.
    pub priority: Option<u8>,
}

impl Run {
    /// A run that survives any width.
    pub fn fixed(text: impl AsRef<str>, style: Style) -> Self {
        Self { text: Rc::from(text.as_ref()), style, priority: None }
    }

    /// A run that goes when the row will not fit.
    pub fn droppable(text: impl AsRef<str>, style: Style, priority: u8) -> Self {
        Self { text: Rc::from(text.as_ref()), style, priority: Some(priority) }
    }

    /// Columns this run takes on screen.
    ///
    /// Cells, not characters. A Japanese file name is two columns per
    /// character, and measuring it as one each puts the status letter past
    /// the right edge, where it is not drawn at all.
    fn width(&self) -> u16 {
        line_index::LineIndex::new(&self.text, 1).width().0 as u16
    }

    /// Cuts this run to `cells` columns, never through a character.
    fn cut(&mut self, cells: u16) {
        let line = line_index::LineIndex::new(&self.text, 1);
        let end = line.cell_to_byte(line_index::CellCol(u32::from(cells)));
        self.text = Rc::from(&self.text[..end.0 as usize]);
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

/// The counts and the change letter, at the right-hand edge.
#[derive(Clone, PartialEq, Eq)]
pub struct Status {
    pub runs: Rc<[Run]>,
}

/// One row.
///
/// Indent is fixed by depth, status by content, and the body takes what is
/// left. When the three will not fit, the droppable runs go in priority
/// order; when they still will not, the widest survivor is cut.
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

    let mut left = Vec::new();
    if !indent.lines.is_empty() {
        left.push(Run::droppable(&indent.lines, indent.style, priority::GUIDES));
    }
    if let Some(icon) = body.icon {
        left.push(Run::fixed(format!("{} ", icon.glyph), base.fg(icon.color)));
    }
    left.extend(body.runs.iter().cloned());

    let right: Vec<Run> = status.as_ref().map(|s| s.runs.to_vec()).unwrap_or_default();

    rsx! {
        Canvas {
            layout: Layout {
                basis: Basis::Length(1),
                shrink: 0,
                fill: Some(base),
                ..Default::default()
            },
            paint: Rc::new(move |paint: &mut loom::Paint<'_>| {
                let area = paint.area();
                cells::fill(paint.cells(), area, base);
                place(paint, left.clone(), right.clone());
            }),
            ..
        }
    }
}

/// Fits the two sides into the row, then writes them.
fn place(paint: &mut loom::Paint<'_>, mut left: Vec<Run>, mut right: Vec<Run>) {
    let area = paint.area();
    let width = area.width;

    while total(&left, &right) > width {
        let Some(level) = lowest(&left, &right) else { break };
        left.retain(|run| run.priority != Some(level));
        right.retain(|run| run.priority != Some(level));
    }

    // A loop, not one pass: cutting the widest run can leave the row still
    // too wide.
    while total(&left, &right) > width && !(left.is_empty() && right.is_empty()) {
        let over = total(&left, &right) - width;
        if !cut_widest(&mut left, &mut right, over) {
            break;
        }
    }

    // The gap takes every spare column, which is what pins the right side to
    // the edge at any width.
    let spare = width.saturating_sub(sum(&left) + sum(&right));
    let gap = if left.is_empty() || right.is_empty() { 0 } else { spare.max(GAP) };

    let mut x = 0;
    for run in &left {
        x = paint.write(area.x + x, area.y, &run.text, run.style) + x;
    }
    x += gap;
    for run in &right {
        x = paint.write(area.x + x, area.y, &run.text, run.style) + x;
    }
}

fn sum(runs: &[Run]) -> u16 {
    runs.iter().map(Run::width).sum()
}

fn total(left: &[Run], right: &[Run]) -> u16 {
    let gap = if left.is_empty() || right.is_empty() { 0 } else { GAP };
    sum(left) + sum(right) + gap
}

fn lowest(left: &[Run], right: &[Run]) -> Option<u8> {
    left.iter().chain(right).filter_map(|run| run.priority).min()
}

/// Cuts the widest run by up to `over` columns. Whether anything was cut.
fn cut_widest(left: &mut [Run], right: &mut [Run], over: u16) -> bool {
    let Some(run) = left
        .iter_mut()
        .chain(right.iter_mut())
        .filter(|run| run.width() > 0)
        .max_by_key(|run| run.width())
    else {
        return false;
    };
    let keep = run.width().saturating_sub(over);
    run.cut(keep);
    true
}
