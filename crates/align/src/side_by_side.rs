//! View lines for the side-by-side layout.

use diff_types::{DetailedLineRangeMapping, LinesDiff};
use file_types::DiffVersion;

use crate::view_line::{Slot, ViewLine};

/// Every paired view line.
pub fn view_lines<'a>(
    diff: &'a LinesDiff,
    original: &'a [String],
    original_lines: u32,
    modified_lines: u32,
) -> ViewLines<'a> {
    ViewLines {
        changes: &diff.changes,
        original_text: original,
        next_change: 0,
        original_lines,
        modified_lines,
        original: 1,
        modified: 1,
        rows: Vec::new(),
        within: 0,
    }
}

/// The number of view lines [`view_lines`] yields.
pub fn view_line_count(
    diff: &LinesDiff,
    original: &[String],
    original_lines: u32,
    modified_lines: u32,
) -> u32 {
    let changed_original: u32 = diff.changes.iter().map(|c| c.original.len()).sum();
    let unchanged = original_lines.saturating_sub(changed_original);
    let changed: u32 = diff.changes.iter().map(|c| height(c, original)).sum();
    let _ = modified_lines;
    unchanged + changed
}

fn height(change: &DetailedLineRangeMapping, original: &[String]) -> u32 {
    if change.inner_changes.is_empty() {
        return change.original.len().max(change.modified.len());
    }
    let mut total = 0;
    let mut from = (change.original.start_line, change.modified.start_line);
    for cut in cuts(change, original) {
        total += (cut.0 - from.0).max(cut.1 - from.1);
        from = cut;
    }
    total
}

/// Exclusive line ends where corresponding text realigns the two sides.
fn cuts(change: &DetailedLineRangeMapping, original: &[String]) -> Vec<(u32, u32)> {
    let mut out = Vec::new();
    let mut last = (change.original.start_line, change.modified.start_line);
    let mut first = true;
    for inner in &change.inner_changes {
        if inner.original.start_col > 1 && inner.modified.start_col > 1 {
            emit_alignment(
                &mut out,
                &mut last,
                &mut first,
                (inner.original.start_line, inner.modified.start_line),
                false,
            );
        }
        if ends_within_its_line(inner.original.end_line, inner.original.end_col, original) {
            emit_alignment(
                &mut out,
                &mut last,
                &mut first,
                (inner.original.end_line, inner.modified.end_line),
                false,
            );
        }
    }
    emit_alignment(
        &mut out,
        &mut last,
        &mut first,
        (change.original.end_line, change.modified.end_line),
        true,
    );
    out
}

fn emit_alignment(
    alignments: &mut Vec<(u32, u32)>,
    last: &mut (u32, u32),
    first: &mut bool,
    next: (u32, u32),
    force: bool,
) {
    if next.0 < last.0 || next.1 < last.1 {
        return;
    }
    if *first {
        *first = false;
    } else if !force && (next.0 == last.0 || next.1 == last.1) {
        return;
    }
    if next == *last {
        return;
    }
    alignments.push(next);
    *last = next;
}

/// Whether an inner change ends before its line does.
fn ends_within_its_line(line: u32, end_col: u32, original: &[String]) -> bool {
    let Some(text) = line.checked_sub(1).and_then(|n| original.get(n as usize)) else {
        return false;
    };
    end_col as usize <= text.encode_utf16().count()
}

fn rows(change: &DetailedLineRangeMapping, original: &[String]) -> Vec<ViewLine> {
    let (os, ms) = (change.original.start_line, change.modified.start_line);

    if change.inner_changes.is_empty() {
        let (olen, mlen) = (change.original.len(), change.modified.len());
        let tall = olen.max(mlen);
        return (0..tall)
            .map(|i| {
                ViewLine::new(
                    slot_from_end(os, olen, i, tall),
                    slot_from_end(ms, mlen, i, tall),
                )
            })
            .collect();
    }

    let mut out = Vec::new();
    let mut from = (os, ms);
    for cut in cuts(change, original) {
        let (olen, mlen) = (cut.0 - from.0, cut.1 - from.1);
        for i in 0..olen.max(mlen) {
            out.push(ViewLine::new(
                slot_at(from.0, olen, i),
                slot_at(from.1, mlen, i),
            ));
        }
        from = cut;
    }
    out
}

/// An iterator over paired view lines.
pub struct ViewLines<'a> {
    changes: &'a [DetailedLineRangeMapping],
    original_text: &'a [String],
    next_change: usize,
    original_lines: u32,
    modified_lines: u32,
    original: u32,
    modified: u32,
    rows: Vec<ViewLine>,
    within: usize,
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

            if self.within == 0 && self.rows.is_empty() {
                self.rows = rows(change, self.original_text);
            }
            if let Some(line) = self.rows.get(self.within) {
                self.within += 1;
                return Some(*line);
            }

            self.original = change.original.end_line;
            self.modified = change.modified.end_line;
            self.rows.clear();
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

fn slot_at(start: u32, len: u32, i: u32) -> Slot {
    if i < len {
        Slot::Line(start + i)
    } else {
        Slot::Filler
    }
}

fn slot_from_end(start: u32, len: u32, i: u32, tall: u32) -> Slot {
    match i.checked_sub(tall - len) {
        Some(n) => Slot::Line(start + n),
        None => Slot::Filler,
    }
}

/// Which file line a view line shows, preferring the modified side.
pub fn line_at(
    diff: &LinesDiff,
    original: &[String],
    original_lines: u32,
    modified_lines: u32,
    view_line: u32,
) -> Option<(DiffVersion, u32)> {
    let line =
        view_lines(diff, original, original_lines, modified_lines).nth(view_line as usize)?;
    match (line.modified.line(), line.original.line()) {
        (Some(line), _) => Some((DiffVersion::Modified, line)),
        (None, Some(line)) => Some((DiffVersion::Original, line)),
        (None, None) => None,
    }
}

/// Which view line shows a file line.
pub fn view_line_at(
    diff: &LinesDiff,
    original: &[String],
    original_lines: u32,
    modified_lines: u32,
    version: DiffVersion,
    line: u32,
) -> Option<u32> {
    view_lines(diff, original, original_lines, modified_lines)
        .position(|l| slot(&l, version).line() == Some(line))
        .map(|n| n as u32)
}

fn slot(line: &ViewLine, version: DiffVersion) -> Slot {
    match version {
        DiffVersion::Original => line.original,
        DiffVersion::Modified => line.modified,
    }
}
