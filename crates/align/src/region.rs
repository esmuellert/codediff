//! The stretches nothing happened in.
//!
//! Derived by inverting the changes, which is how VSCode builds the same thing
//! (`UnchangedRegion.fromDiffs`). A long run of untouched lines can then be
//! collapsed to a single "47 hidden lines" row, keeping a few lines of context
//! on each side of the edits that remain visible.

use vscode_diff::{LineRange, LinesDiff};

/// A run of lines identical on both sides.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct UnchangedRegion {
    pub original: LineRange,
    pub modified: LineRange,
}

impl UnchangedRegion {
    pub fn len(&self) -> u32 {
        self.original.len()
    }

    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// The part still worth collapsing once `context` lines are kept visible at
    /// each end. `None` when the region is too short to be worth hiding.
    pub fn hidden(&self, context: u32, minimum: u32) -> Option<UnchangedRegion> {
        let trimmed = self.len().saturating_sub(context.saturating_mul(2));
        if trimmed < minimum.max(1) {
            return None;
        }
        Some(UnchangedRegion {
            original: LineRange {
                start_line: self.original.start_line + context,
                end_line: self.original.end_line - context,
            },
            modified: LineRange {
                start_line: self.modified.start_line + context,
                end_line: self.modified.end_line - context,
            },
        })
    }
}

/// Every unchanged run, in order, including those before the first change and
/// after the last.
pub fn regions(diff: &LinesDiff, original_lines: u32, modified_lines: u32) -> Vec<UnchangedRegion> {
    let mut out = Vec::new();
    let mut original = 1;
    let mut modified = 1;

    for change in &diff.changes {
        push(
            &mut out,
            original,
            modified,
            change.original.start_line,
            change.modified.start_line,
        );
        original = change.original.end_line;
        modified = change.modified.end_line;
    }
    push(
        &mut out,
        original,
        modified,
        original_lines + 1,
        modified_lines + 1,
    );
    out
}

fn push(
    out: &mut Vec<UnchangedRegion>,
    original: u32,
    modified: u32,
    original_end: u32,
    modified_end: u32,
) {
    if original_end > original {
        out.push(UnchangedRegion {
            original: LineRange {
                start_line: original,
                end_line: original_end,
            },
            modified: LineRange {
                start_line: modified,
                end_line: modified_end,
            },
        });
    }
}
