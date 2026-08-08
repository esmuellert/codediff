//! The layout that shows one version per view line.
//!
//! Every line of both versions gets a view line of its own: unchanged lines once,
//! since both versions agree on them, then a change's deleted lines followed
//! by its inserted ones. A change is therefore as tall as the sum of its
//! sides, where [`crate::side_by_side`] makes it as tall as the taller one.
//! That single difference is the entire layout.
//!
//! The view lines are the same [`ViewLine`]s. A deleted line is `(Line, Filler)` and an
//! inserted line is `(Filler, Line)` — shapes the paired space already emits —
//! so [`ViewLineType`](crate::ViewLineType) is derived unchanged, inner-change spans are
//! looked up unchanged, and everything downstream works here without a branch.
//! What the paired space would show as one modified row, this shows as two.
//!
//! Nothing is stored and nothing is copied: like its sibling, this walks the
//! changes and yields rows on demand.

use diff_types::{DetailedLineRangeMapping, LinesDiff};
use file_types::DiffVersion;

use crate::view_line::{Slot, ViewLine};

/// Every view line of the unified document, in order.
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
/// Unchanged lines appear once; a change contributes both of its sides. Only
/// meaningful for a diff satisfying [`crate::is_well_formed`].
pub fn view_line_count(diff: &LinesDiff, original_lines: u32, modified_lines: u32) -> u32 {
    let changed_original: u32 = diff.changes.iter().map(|c| c.original.len()).sum();
    let unchanged = original_lines.saturating_sub(changed_original);
    let changed: u32 = diff.changes.iter().map(height).sum();
    let _ = modified_lines;
    unchanged + changed
}

/// A change takes as many rows as it has lines, on both sides together.
fn height(change: &DetailedLineRangeMapping) -> u32 {
    change.original.len() + change.modified.len()
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
            // Both versions have it, so it is one view line, not two.
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
                // Deletions first, then insertions: what was there, then what
                // replaced it. Reading the other order would describe an edit
                // backwards.
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

        // Past the last change: whatever remains is unchanged and pairs up.
        if self.original <= self.original_lines && self.modified <= self.modified_lines {
            let line = ViewLine::unchanged(self.original, self.modified);
            self.original += 1;
            self.modified += 1;
            return Some(line);
        }
        None
    }
}

/// Which line of which version a view line shows.
///
/// Unambiguous here in a way it is not side by side: every view line belongs to one
/// version, except an unchanged view line, which both versions agree on.
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
