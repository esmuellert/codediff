//! Which way the view lines run.
//!
//! A diff can be walked in more than one way, and the choice changes what
//! "view line 40" means — so it cannot be a detail hidden inside a renderer.
//! The choice itself is [`DiffType`], in `file-types`, because every layer
//! from the pipeline to the renderer has to name it. What is here is the walk:
//! the iterator that lets both of them be returned from one function.
//!
//! The choice used to live here, as `DiffLayout`, on the grounds that it is
//! the parameter to the question this crate answers. It also had to say
//! "single file", and could not: a single file is not a way of walking a
//! pairing. Four `Option`s in three crates spelled that absence instead. See
//! D60.
//!
//! The cost of that move is here, and it is four lines: an [`Alignment`] is
//! built only for a file with two sides, so `DiffType::Single` cannot reach a
//! walk — but the type permits it, and each function says so with an
//! `unreachable!`. Measured: the whole suite runs without reaching one.
//!
//! [`Alignment`]: crate::Alignment
//!
//! The pane arrangement `ui` calls a `Layout` is a different thing at a
//! different scale, settled long after this one.

use diff_types::LinesDiff;
use file_types::{DiffType, DiffVersion};

use crate::view_line::ViewLine;
use crate::{inline, side_by_side};

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
    diff_type: DiffType,
    diff: &'a LinesDiff,
    original: &'a [String],
    original_lines: u32,
    modified_lines: u32,
) -> ViewLines<'a> {
    match diff_type {
        DiffType::SideBySide => ViewLines::SideBySide(side_by_side::view_lines(
            diff,
            original,
            original_lines,
            modified_lines,
        )),
        DiffType::Inline => {
            ViewLines::Inline(inline::view_lines(diff, original_lines, modified_lines))
        }
        // An `Alignment` is built only for a file that has two sides, so a
        // single file never reaches a walk. See D60.
        DiffType::Single => unreachable!("a single file has no pairing to walk"),
    }
}

pub(crate) fn view_line_count(
    diff_type: DiffType,
    diff: &LinesDiff,
    original: &[String],
    original_lines: u32,
    modified_lines: u32,
) -> u32 {
    match diff_type {
        DiffType::SideBySide => {
            side_by_side::view_line_count(diff, original, original_lines, modified_lines)
        }
        DiffType::Inline => inline::view_line_count(diff, original_lines, modified_lines),
        // An `Alignment` is built only for a file that has two sides, so a
        // single file never reaches a walk. See D60.
        DiffType::Single => unreachable!("a single file has no pairing to walk"),
    }
}

pub(crate) fn line_at(
    diff_type: DiffType,
    diff: &LinesDiff,
    original: &[String],
    original_lines: u32,
    modified_lines: u32,
    view_line: u32,
) -> Option<(DiffVersion, u32)> {
    match diff_type {
        DiffType::SideBySide => {
            side_by_side::line_at(diff, original, original_lines, modified_lines, view_line)
        }
        DiffType::Inline => inline::line_at(diff, original_lines, modified_lines, view_line),
        // An `Alignment` is built only for a file that has two sides, so a
        // single file never reaches a walk. See D60.
        DiffType::Single => unreachable!("a single file has no pairing to walk"),
    }
}

pub(crate) fn view_line_at(
    diff_type: DiffType,
    diff: &LinesDiff,
    original: &[String],
    original_lines: u32,
    modified_lines: u32,
    version: DiffVersion,
    line: u32,
) -> Option<u32> {
    match diff_type {
        DiffType::SideBySide => side_by_side::view_line_at(
            diff,
            original,
            original_lines,
            modified_lines,
            version,
            line,
        ),
        DiffType::Inline => {
            inline::view_line_at(diff, original_lines, modified_lines, version, line)
        }
        // An `Alignment` is built only for a file that has two sides, so a
        // single file never reaches a walk. See D60.
        DiffType::Single => unreachable!("a single file has no pairing to walk"),
    }
}
