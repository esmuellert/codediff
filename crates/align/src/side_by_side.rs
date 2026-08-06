//! The layout that shows both versions at once.
//!
//! Each row carries a slot for each version: two lines that correspond, or a
//! line against a filler where one side has nothing. The two versions stay
//! level down the screen — which is the whole point of the layout, and the
//! reason two columns drawn from one view-line slice cannot drift apart.
//!
//! **A change is split before its fillers are placed.** Where the engine found
//! character-level detail, the lines it matched are pulled level with each
//! other and the fillers go around them, so a line that survived a rewrite
//! sits beside itself rather than beside whatever happens to be that far down.
//! Without that detail — a whole block inserted or deleted — the fillers go at
//! the start of the block. Both are what `codediff.nvim` does, and what VS
//! Code does before it.
//!
//! Compare [`crate::inline`], which gives every line a view line of its own.

use diff_types::{DetailedLineRangeMapping, LinesDiff};
use file_types::DiffVersion;

use crate::view_line::{Slot, ViewLine};

/// Every view line of the paired document, in order.
///
/// `original` is the text of the original side, needed to tell an inner change
/// that ends mid-line from one that runs to the end of it. `original_lines` and
/// `modified_lines` are the line counts, needed to emit the unchanged run after
/// the last change.
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

/// The number of view lines [`view_lines`] will yield.
///
/// Unchanged lines pair one to one; a change is as tall as its splits make it.
/// Only meaningful for a diff satisfying [`crate::is_well_formed`].
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

/// How many view lines a change occupies.
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

/// Where a change is cut into runs that line up, as `(original, modified)`
/// exclusive ends.
///
/// The cuts come from the inner changes: one before an inner change that starts
/// part way into both lines, and one after an inner change that ends before the
/// end of its line. Either says the text around it corresponds, so the lines
/// carrying it can be pulled level. A cut that would leave one side empty is
/// dropped, except the first — which is what keeps a leading insertion whole.
fn cuts(change: &DetailedLineRangeMapping, original: &[String]) -> Vec<(u32, u32)> {
    let mut candidates = Vec::new();
    for inner in &change.inner_changes {
        if inner.original.start_col > 1 && inner.modified.start_col > 1 {
            candidates.push((inner.original.start_line, inner.modified.start_line));
        }
        if ends_within_its_line(inner.original.end_line, inner.original.end_col, original) {
            candidates.push((inner.original.end_line, inner.modified.end_line));
        }
    }
    candidates.push((change.original.end_line, change.modified.end_line));

    let mut out = Vec::new();
    let (mut last_o, mut last_m) = (change.original.start_line, change.modified.start_line);
    let mut first = true;
    for (o, m) in candidates {
        if o < last_o || m < last_m {
            continue;
        }
        if first {
            first = false;
        } else if o == last_o || m == last_m {
            continue;
        }
        if o > last_o || m > last_m {
            out.push((o, m));
        }
        (last_o, last_m) = (o, m);
    }
    out
}

/// Whether an inner change stops before the end of the line it ends on.
///
/// Measured in UTF-16 units because that is what the engine counts in.
fn ends_within_its_line(line: u32, end_col: u32, original: &[String]) -> bool {
    let Some(text) = line.checked_sub(1).and_then(|n| original.get(n as usize)) else {
        return false;
    };
    end_col as usize <= text.encode_utf16().count()
}

/// The rows one change occupies, fillers included.
fn rows(change: &DetailedLineRangeMapping, original: &[String]) -> Vec<ViewLine> {
    let (os, ms) = (change.original.start_line, change.modified.start_line);

    if change.inner_changes.is_empty() {
        // Nothing corresponds, so nothing can be pulled level. The fillers go
        // at the start of the block, which is where a reader looks for the
        // line the block replaced.
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

/// Walks the changes, expanding each into rows on demand.
pub struct ViewLines<'a> {
    changes: &'a [DetailedLineRangeMapping],
    original_text: &'a [String],
    next_change: usize,
    original_lines: u32,
    modified_lines: u32,
    /// The next original line not yet emitted, 1-based.
    original: u32,
    /// The next modified line not yet emitted, 1-based.
    modified: u32,
    /// The current change's rows, built when it is reached.
    rows: Vec<ViewLine>,
    /// How far into `rows` we are.
    within: usize,
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

/// The `i`th row of a range whose fillers come first rather than last.
fn slot_from_end(start: u32, len: u32, i: u32, tall: u32) -> Slot {
    match i.checked_sub(tall - len) {
        Some(n) => Slot::Line(start + n),
        None => Slot::Filler,
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

/// Which row shows a given line, if any.
///
/// The other half. A walk rather than an index because it runs when the reader
/// presses a key, not when a frame is drawn.
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
