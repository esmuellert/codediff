//! The layout that shows both versions at once.
//!
//! Each row carries a slot for each version: two lines that correspond, or a
//! line against a filler where one side has nothing. A change is therefore as
//! tall as its **taller** side, and the two versions stay level down the
//! screen — which is the whole point of the layout, and the reason two columns
//! drawn from one view-line slice cannot drift apart.
//!
//! Compare [`crate::inline`], which gives every line a view line of its own.

use diff_types::{DetailedLineRangeMapping, LinesDiff};
use file_types::DiffVersion;

use crate::view_line::{Slot, ViewLine};

/// Every view line of the paired document, in order.
///
/// `original_lines` and `modified_lines` are the line counts of the two files,
/// needed only to emit the unchanged run after the last change.
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

/// The number of view lines [`rows`] will yield.
///
/// Unchanged lines pair one to one, and a change is as tall as its taller side.
/// Only meaningful for a diff satisfying [`crate::is_well_formed`].
pub fn view_line_count(diff: &LinesDiff, original_lines: u32, modified_lines: u32) -> u32 {
    let changed_original: u32 = diff.changes.iter().map(|c| c.original.len()).sum();
    let unchanged = original_lines.saturating_sub(changed_original);
    let changed: u32 = diff.changes.iter().map(tallest).sum();
    let _ = modified_lines;
    unchanged + changed
}

fn tallest(change: &DetailedLineRangeMapping) -> u32 {
    change.original.len().max(change.modified.len())
}

/// Walks the changes, expanding each into rows on demand.
pub struct ViewLines<'a> {
    changes: &'a [DetailedLineRangeMapping],
    next_change: usize,
    original_lines: u32,
    modified_lines: u32,
    /// The next original line not yet emitted, 1-based.
    original: u32,
    /// The next modified line not yet emitted, 1-based.
    modified: u32,
    /// How far into the current change we are.
    within: u32,
}

impl Iterator for ViewLines<'_> {
    type Item = ViewLine;

    fn next(&mut self) -> Option<ViewLine> {
        while let Some(change) = self.changes.get(self.next_change) {
            // The unchanged run leading up to this change, one view line per line.
            if self.original < change.original.start_line {
                let line = ViewLine::unchanged(self.original, self.modified);
                self.original += 1;
                self.modified += 1;
                return Some(line);
            }

            let height = tallest(change);
            if self.within < height {
                let i = self.within;
                self.within += 1;
                return Some(ViewLine::new(
                    slot_at(change.original.start_line, change.original.len(), i),
                    slot_at(change.modified.start_line, change.modified.len(), i),
                ));
            }

            self.original = change.original.end_line;
            self.modified = change.modified.end_line;
            self.within = 0;
            self.next_change += 1;
        }

        // Past the last change: whatever remains pairs one to one.
        if self.original <= self.original_lines && self.modified <= self.modified_lines {
            let line = ViewLine::unchanged(self.original, self.modified);
            self.original += 1;
            self.modified += 1;
            return Some(line);
        }
        None
    }
}

/// The `i`th line of a range, or a filler once the range runs out.
fn slot_at(start: u32, len: u32, i: u32) -> Slot {
    if i < len {
        Slot::Line(start + i)
    } else {
        Slot::Filler
    }
}

/// Which line of which version a view line shows.
///
/// Half of the pair that lets a reader keep their place when the layout
/// changes: a view-line number means nothing in the other view layout, but a *line*
/// means the same thing in both. Prefers the modified version, since that is
/// the one being reviewed; falls back to the original where the modified side
/// has nothing.
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
///
/// The other half. A walk rather than an index because it runs when the reader
/// presses a key, not when a frame is drawn.
pub fn view_line_at(
    diff: &LinesDiff,
    original_lines: u32,
    modified_lines: u32,
    version: DiffVersion,
    line: u32,
) -> Option<u32> {
    view_lines(diff, original_lines, modified_lines)
        .position(|l| slot(&l, version).line() == Some(line))
        .map(|n| n as u32)
}

fn slot(line: &ViewLine, version: DiffVersion) -> Slot {
    match version {
        DiffVersion::Original => line.original,
        DiffVersion::Modified => line.modified,
    }
}
