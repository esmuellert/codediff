//! The `ViewLine` type shared by both layouts.
//!
//! The vocabulary only. The two layouts that use it live beside this
//! file — [`side_by_side`] pairs the versions across a view line, [`inline`] gives
//! each version a view line of its own — and they share these types precisely so
//! that everything downstream of a view line works in both without a branch.
//!
//! Nothing here is stored. A layout walks the changes and yields one view line at
//! a time, which is why a ten-thousand-line file costs nothing more than a
//! ten-line one until something asks to draw it.
//!
//! [`side_by_side`]: crate::side_by_side
//! [`inline`]: crate::inline

use std::ops::Range;

use diff_types::LinesDiff;

/// What a side shows on one view line.
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

/// What a view line shows, which follows from its two slots.
///
/// A move is not a variant here. A moved block is reported by the engine as an
/// ordinary deletion and an ordinary insertion whose ranges need not line up
/// with either, so it cannot be a kind of row; ask [`crate::Alignment::moved`]
/// instead. VSCode reached the same conclusion and deleted the equivalent
/// fields from `DiffMapping`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ViewLineType {
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
pub struct ViewLine {
    pub original: Slot,
    pub modified: Slot,
    pub kind: ViewLineType,
}

impl ViewLine {
    pub(crate) fn new(original: Slot, modified: Slot) -> Self {
        let kind = match (original, modified) {
            (Slot::Line(_), Slot::Filler) => ViewLineType::Deleted,
            (Slot::Filler, Slot::Line(_)) => ViewLineType::Inserted,
            _ => ViewLineType::Modified,
        };
        Self {
            original,
            modified,
            kind,
        }
    }

    pub(crate) fn unchanged(original: u32, modified: u32) -> Self {
        Self {
            original: Slot::Line(original),
            modified: Slot::Line(modified),
            kind: ViewLineType::Unchanged,
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
/// make a layout walk backwards and emit a line twice while `view_line_count`, which
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

/// Adjacent changed view lines grouped into row ranges.
///
/// Used by both change navigation (`]c`/`[c`) and the status line count.
/// Not the same as [`crate::Hunk`] — a hunk merges nearby changes, but
/// navigation needs each edit as its own stop.
/// Generic over the iterator so both layouts use one function.
pub fn blocks(view_lines: impl Iterator<Item = ViewLine>) -> Vec<Range<u32>> {
    let mut blocks: Vec<Range<u32>> = Vec::new();
    for (index, line) in view_lines.enumerate() {
        let index = index as u32;
        if line.kind == ViewLineType::Unchanged {
            continue;
        }
        match blocks.last_mut() {
            Some(last) if last.end == index => last.end = index + 1,
            _ => blocks.push(index..index + 1),
        }
    }
    blocks
}
