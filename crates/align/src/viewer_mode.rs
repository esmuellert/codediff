//! Side-by-side vs inline: two ways to turn a diff into view lines.
//!
//! Parameterised by [`ViewerMode`]. `Single` has no pairing, so it never
//! reaches here.

use diff_types::LinesDiff;
use file_types::{DiffVersion, ViewerMode};

use crate::view_line::ViewLine;
use crate::{inline, side_by_side};

/// Iterator over either view layout.
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
    viewer_mode: ViewerMode,
    diff: &'a LinesDiff,
    original: &'a [String],
    original_lines: u32,
    modified_lines: u32,
) -> ViewLines<'a> {
    match viewer_mode {
        ViewerMode::SideBySide => ViewLines::SideBySide(side_by_side::view_lines(
            diff,
            original,
            original_lines,
            modified_lines,
        )),
        ViewerMode::Inline => {
            ViewLines::Inline(inline::view_lines(diff, original_lines, modified_lines))
        }
        ViewerMode::Single => unreachable!("a single file has no pairing to walk"),
    }
}

pub(crate) fn view_line_count(
    viewer_mode: ViewerMode,
    diff: &LinesDiff,
    original: &[String],
    original_lines: u32,
    modified_lines: u32,
) -> u32 {
    match viewer_mode {
        ViewerMode::SideBySide => {
            side_by_side::view_line_count(diff, original, original_lines, modified_lines)
        }
        ViewerMode::Inline => inline::view_line_count(diff, original_lines, modified_lines),
        ViewerMode::Single => unreachable!("a single file has no pairing to walk"),
    }
}

pub(crate) fn line_at(
    viewer_mode: ViewerMode,
    diff: &LinesDiff,
    original: &[String],
    original_lines: u32,
    modified_lines: u32,
    view_line: u32,
) -> Option<(DiffVersion, u32)> {
    match viewer_mode {
        ViewerMode::SideBySide => {
            side_by_side::line_at(diff, original, original_lines, modified_lines, view_line)
        }
        ViewerMode::Inline => inline::line_at(diff, original_lines, modified_lines, view_line),
        ViewerMode::Single => unreachable!("a single file has no pairing to walk"),
    }
}

pub(crate) fn view_line_at(
    viewer_mode: ViewerMode,
    diff: &LinesDiff,
    original: &[String],
    original_lines: u32,
    modified_lines: u32,
    version: DiffVersion,
    line: u32,
) -> Option<u32> {
    match viewer_mode {
        ViewerMode::SideBySide => side_by_side::view_line_at(
            diff,
            original,
            original_lines,
            modified_lines,
            version,
            line,
        ),
        ViewerMode::Inline => {
            inline::view_line_at(diff, original_lines, modified_lines, version, line)
        }
        ViewerMode::Single => unreachable!("a single file has no pairing to walk"),
    }
}
