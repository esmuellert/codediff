//! Which way the view lines run.
//!
//! A diff can be walked in more than one way, and the choice changes what
//! "view line 40" means — so it cannot be a detail hidden inside a renderer.
//! This is the choice, named, plus the iterator that lets both walks be
//! returned from one function.
//!
//! **The one place `SideBySide` and `Inline` are defined.** `ui` names this
//! type rather than repeating the words: a buffer holds a `DiffLayout`, and so
//! does the keymap selector. `ui` depends on `align`, so this is the lowest
//! crate both can reach — a leaf crate would be for words shared by layers
//! that *cannot* see each other, which is why `DiffVersion` sank to
//! `file-types` and this did not. See D33.
//!
//! It belongs here rather than in `ui` because it is the parameter to the
//! question this crate exists to answer: which line appears where. The pane
//! arrangement `ui` calls a `Layout` is a different thing at a different
//! scale, settled long after this one.

use diff_types::LinesDiff;
use file_types::DiffVersion;

use crate::view_line::ViewLine;
use crate::{inline, side_by_side};

/// One of the ways a diff can be laid out as view_lines.
///
/// The two are not variations on a theme: they produce different view-line counts
/// from the same diff, so a position in one is meaningless in the other. That
/// is why the buffer showing a diff is a different buffer per view layout, and
/// why switching between them has to translate through a line number.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum DiffLayout {
    /// Both versions at once, a view line carrying a slot for each.
    #[default]
    SideBySide,
    /// One version per view line: what was deleted, then what replaced it.
    Inline,
}

impl DiffLayout {
    /// The other one, which is what a toggle asks for.
    pub fn other(self) -> Self {
        match self {
            DiffLayout::SideBySide => DiffLayout::Inline,
            DiffLayout::Inline => DiffLayout::SideBySide,
        }
    }
}

/// ViewLines from either walk.
///
/// An enum rather than a boxed iterator: the two walks have different types,
/// and a frame should not allocate to ask for its view_lines.
pub enum ViewLines<'a> {
    SideBySide(side_by_side::ViewLines<'a>),
    Inline(inline::ViewLines<'a>),
}

impl Iterator for ViewLines<'_> {
    type Item = ViewLine;

    fn next(&mut self) -> Option<ViewLine> {
        match self {
            ViewLines::SideBySide(lines) => lines.next(),
            ViewLines::Inline(lines) => lines.next(),
        }
    }
}

pub(crate) fn view_lines<'a>(
    layout: DiffLayout,
    diff: &'a LinesDiff,
    original: &'a [String],
    original_lines: u32,
    modified_lines: u32,
) -> ViewLines<'a> {
    match layout {
        DiffLayout::SideBySide => ViewLines::SideBySide(side_by_side::view_lines(
            diff,
            original,
            original_lines,
            modified_lines,
        )),
        DiffLayout::Inline => {
            ViewLines::Inline(inline::view_lines(diff, original_lines, modified_lines))
        }
    }
}

pub(crate) fn view_line_count(
    layout: DiffLayout,
    diff: &LinesDiff,
    original: &[String],
    original_lines: u32,
    modified_lines: u32,
) -> u32 {
    match layout {
        DiffLayout::SideBySide => {
            side_by_side::view_line_count(diff, original, original_lines, modified_lines)
        }
        DiffLayout::Inline => inline::view_line_count(diff, original_lines, modified_lines),
    }
}

pub(crate) fn line_at(
    layout: DiffLayout,
    diff: &LinesDiff,
    original: &[String],
    original_lines: u32,
    modified_lines: u32,
    view_line: u32,
) -> Option<(DiffVersion, u32)> {
    match layout {
        DiffLayout::SideBySide => {
            side_by_side::line_at(diff, original, original_lines, modified_lines, view_line)
        }
        DiffLayout::Inline => inline::line_at(diff, original_lines, modified_lines, view_line),
    }
}

pub(crate) fn view_line_at(
    layout: DiffLayout,
    diff: &LinesDiff,
    original: &[String],
    original_lines: u32,
    modified_lines: u32,
    version: DiffVersion,
    line: u32,
) -> Option<u32> {
    match layout {
        DiffLayout::SideBySide => side_by_side::view_line_at(
            diff,
            original,
            original_lines,
            modified_lines,
            version,
            line,
        ),
        DiffLayout::Inline => {
            inline::view_line_at(diff, original_lines, modified_lines, version, line)
        }
    }
}
