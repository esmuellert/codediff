//! View lines for the inline layout.

use diff_types::{DetailedLineRangeMapping, LinesDiff};
use file_types::DiffVersion;

use crate::view_line::{Slot, ViewLine};

/// Every view line of the unified document.
pub fn view_lines(diff: &LinesDiff, original_lines: u32, modified_lines: u32) -> ViewLines<'_> {
    ViewLines {
        changes: &diff.changes,
        next_change: 0,
        original_lines,
        modified_lines,
        original: 1,
        modified: 1,
        within: 0,
    }
}

/// The number of view lines [`view_lines`] yields.
pub fn view_line_count(diff: &LinesDiff, original_lines: u32, modified_lines: u32) -> u32 {
    let changed_original: u32 = diff.changes.iter().map(|c| c.original.len()).sum();
    let unchanged = original_lines.saturating_sub(changed_original);
    let changed: u32 = diff.changes.iter().map(height).sum();
    let _ = modified_lines;
    unchanged + changed
}

fn height(change: &DetailedLineRangeMapping) -> u32 {
    change.original.len() + change.modified.len()
}

/// An iterator over inline view lines.
pub struct ViewLines<'a> {
    changes: &'a [DetailedLineRangeMapping],
    next_change: usize,
    original_lines: u32,
    modified_lines: u32,
    original: u32,
    modified: u32,
    within: u32,
}

impl Iterator for ViewLines<'_> {
    type Item = ViewLine;

    fn next(&mut self) -> Option<ViewLine> {
        while let Some(change) = self.changes.get(self.next_change) {
            if self.original < change.original.start_line {
                let line = ViewLine::unchanged(self.original, self.modified);
                self.original += 1;
                self.modified += 1;
                return Some(line);
            }

            let deleted = change.original.len();
            if self.within < height(change) {
                let i = self.within;
                self.within += 1;
                return Some(if i < deleted {
                    ViewLine::new(Slot::Line(change.original.start_line + i), Slot::Filler)
                } else {
                    ViewLine::new(
                        Slot::Filler,
                        Slot::Line(change.modified.start_line + (i - deleted)),
                    )
                });
            }

            self.original = change.original.end_line;
            self.modified = change.modified.end_line;
            self.within = 0;
            self.next_change += 1;
        }

        if self.original <= self.original_lines && self.modified <= self.modified_lines {
            let line = ViewLine::unchanged(self.original, self.modified);
            self.original += 1;
            self.modified += 1;
            return Some(line);
        }
        None
    }
}

/// Which file line a view line shows.
pub fn line_at(
    diff: &LinesDiff,
    original_lines: u32,
    modified_lines: u32,
    view_line: u32,
) -> Option<(DiffVersion, u32)> {
    let line = view_lines(diff, original_lines, modified_lines).nth(view_line as usize)?;
    match (line.modified.line(), line.original.line()) {
        (Some(line), _) => Some((DiffVersion::Modified, line)),
        (None, Some(line)) => Some((DiffVersion::Original, line)),
        (None, None) => None,
    }
}

/// Which row shows a given line, if any.
pub fn view_line_at(
    diff: &LinesDiff,
    original_lines: u32,
    modified_lines: u32,
    version: DiffVersion,
    line: u32,
) -> Option<u32> {
    let slot = |line: &ViewLine| match version {
        DiffVersion::Original => line.original,
        DiffVersion::Modified => line.modified,
    };
    view_lines(diff, original_lines, modified_lines)
        .position(|l| slot(&l).line() == Some(line))
        .map(|n| n as u32)
}
