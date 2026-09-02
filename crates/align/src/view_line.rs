//! View-line types shared by both layouts.

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

/// What a view line shows. Move metadata is queried separately.
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
    pub fn line_pair(&self) -> Option<(u32, u32)> {
        Some((self.original.line()?, self.modified.line()?))
    }
}

/// Whether both sides are ordered, non-overlapping, and in bounds.
pub fn is_well_formed(diff: &LinesDiff, original_lines: u32, modified_lines: u32) -> bool {
    let (mut original, mut modified) = (1, 1);
    for change in &diff.changes {
        if change.original.start_line < original || change.modified.start_line < modified {
            return false;
        }
        if change.original.end_line < change.original.start_line
            || change.modified.end_line < change.modified.start_line
        {
            return false;
        }
        if change.original.end_line > original_lines + 1
            || change.modified.end_line > modified_lines + 1
        {
            return false;
        }
        if change.original.start_line - original != change.modified.start_line - modified {
            return false;
        }
        original = change.original.end_line;
        modified = change.modified.end_line;
    }
    original_lines + 1 - original == modified_lines + 1 - modified
}

/// Adjacent changed view lines grouped for navigation.
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
