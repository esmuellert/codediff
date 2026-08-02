//! Rows: pairing a line on each side, or a line against a filler.
//!
//! Nothing here is stored. [`rows`] walks the changes and yields one row at a
//! time, which is why a ten-thousand-line file costs nothing more than a
//! ten-line one until something asks to draw it.

use diff_types::{DetailedLineRangeMapping, LinesDiff};

/// What a side shows on one row.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Slot {
    /// A line of that side's file, numbered from 1.
    Line(u32),
    /// Nothing — the other side has a line here and this one does not.
    Filler,
}

impl Slot {
    pub fn line(self) -> Option<u32> {
        match self {
            Slot::Line(n) => Some(n),
            Slot::Filler => None,
        }
    }

    pub fn is_filler(self) -> bool {
        self == Slot::Filler
    }
}

/// What a row shows, which follows from its two slots.
///
/// A move is deliberately absent. A moved block is reported by the engine as an
/// ordinary deletion and an ordinary insertion whose ranges need not line up
/// with either, so it cannot be a kind of row; ask [`crate::Alignment::moved`]
/// instead. VSCode reached the same conclusion and deleted the equivalent
/// fields from `DiffMapping`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RowKind {
    Unchanged,
    /// Both sides have a line and they differ.
    Modified,
    /// Only the original has a line.
    Deleted,
    /// Only the modified has a line.
    Inserted,
}

/// One row of the paired document.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Row {
    pub original: Slot,
    pub modified: Slot,
    pub kind: RowKind,
}

impl Row {
    fn new(original: Slot, modified: Slot) -> Self {
        let kind = match (original, modified) {
            (Slot::Line(_), Slot::Filler) => RowKind::Deleted,
            (Slot::Filler, Slot::Line(_)) => RowKind::Inserted,
            _ => RowKind::Modified,
        };
        Self {
            original,
            modified,
            kind,
        }
    }

    fn unchanged(original: u32, modified: u32) -> Self {
        Self {
            original: Slot::Line(original),
            modified: Slot::Line(modified),
            kind: RowKind::Unchanged,
        }
    }

    /// The line each side shows, if both do.
    pub fn both(&self) -> Option<(u32, u32)> {
        Some((self.original.line()?, self.modified.line()?))
    }
}

/// Whether the changes are shaped the way every later step assumes.
///
/// Both sides must be ordered, non-overlapping and within their file. The
/// engine guarantees this — `align`'s fixture tests check it over all twelve
/// vendored pairs — but a diff that broke it would not fail loudly. It would
/// make [`rows`] walk backwards and emit a line twice while `row_count`, which
/// only sums lengths, kept reporting the smaller figure. Silent duplication in
/// a review tool is worse than a refusal.
pub fn is_well_formed(diff: &LinesDiff, original_lines: u32, modified_lines: u32) -> bool {
    let (mut original, mut modified) = (1, 1);
    for change in &diff.changes {
        if change.original.start_line < original || change.modified.start_line < modified {
            return false; // out of order, or overlapping its predecessor
        }
        if change.original.end_line < change.original.start_line
            || change.modified.end_line < change.modified.start_line
        {
            return false; // inverted
        }
        if change.original.end_line > original_lines + 1
            || change.modified.end_line > modified_lines + 1
        {
            return false; // past the end of its file
        }
        // The unchanged run before a change pairs one to one, so both sides
        // must have advanced by the same amount.
        if change.original.start_line - original != change.modified.start_line - modified {
            return false;
        }
        original = change.original.end_line;
        modified = change.modified.end_line;
    }
    original_lines + 1 - original == modified_lines + 1 - modified
}

/// Every row of the paired document, in order.
///
/// `original_lines` and `modified_lines` are the line counts of the two files,
/// needed only to emit the unchanged run after the last change.
pub fn rows(diff: &LinesDiff, original_lines: u32, modified_lines: u32) -> Rows<'_> {
    Rows {
        changes: &diff.changes,
        next_change: 0,
        original_lines,
        modified_lines,
        original: 1,
        modified: 1,
        within: 0,
    }
}

/// The number of rows [`rows`] will yield.
///
/// Unchanged lines pair one to one, and a change is as tall as its taller side.
/// Only meaningful for a diff satisfying [`is_well_formed`].
pub fn row_count(diff: &LinesDiff, original_lines: u32, modified_lines: u32) -> u32 {
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
pub struct Rows<'a> {
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

impl Iterator for Rows<'_> {
    type Item = Row;

    fn next(&mut self) -> Option<Row> {
        while let Some(change) = self.changes.get(self.next_change) {
            // The unchanged run leading up to this change, one row per line.
            if self.original < change.original.start_line {
                let row = Row::unchanged(self.original, self.modified);
                self.original += 1;
                self.modified += 1;
                return Some(row);
            }

            let height = tallest(change);
            if self.within < height {
                let i = self.within;
                self.within += 1;
                return Some(Row::new(
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
            let row = Row::unchanged(self.original, self.modified);
            self.original += 1;
            self.modified += 1;
            return Some(row);
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
