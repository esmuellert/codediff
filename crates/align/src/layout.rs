//! Side-by-side vs inline: two ways to turn a diff into view lines.
//!
//! Parameterised by [`DiffType`]. `Single` has no pairing, so it never
//! reaches here.

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
