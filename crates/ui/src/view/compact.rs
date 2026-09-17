//! Selects logical diff lines for compact presentation.

use std::ops::Range;

use align::{Alignment, ViewLine, ViewLineContent, ViewLineType};
use diff_types::LineRange;
use file_types::{DiffType, DiffVersion};

const COMPACT_CONTEXT_LINES: u32 = 3;

pub(crate) fn view_lines(
    alignment: &Alignment,
    diff_type: DiffType,
    compact: bool,
) -> Vec<ViewLine> {
    let view_lines = alignment.view_lines(diff_type).collect::<Vec<_>>();
    if compact {
        compact_view_lines(alignment, view_lines)
    } else {
        view_lines
    }
}

fn compact_view_lines(alignment: &Alignment, view_lines: Vec<ViewLine>) -> Vec<ViewLine> {
    if alignment.is_empty() {
        return view_lines;
    }

    let original_ranges = compact_ranges(
        alignment.hunks().iter().map(|hunk| hunk.original),
        alignment.lines(DiffVersion::Original).len() as u32,
    );
    let modified_ranges = compact_ranges(
        alignment.hunks().iter().map(|hunk| hunk.modified),
        alignment.lines(DiffVersion::Modified).len() as u32,
    );
    let mut compact = Vec::with_capacity(view_lines.len());
    let mut hidden = false;

    for view_line in view_lines {
        let visible = view_line.kind != ViewLineType::Unchanged
            || source_line_in_ranges(view_line.original, &original_ranges)
            || source_line_in_ranges(view_line.modified, &modified_ranges);
        if visible {
            if hidden {
                compact.push(fold_marker_view_line());
            }
            compact.push(view_line);
            hidden = false;
        } else {
            hidden = true;
        }
    }

    compact
}

fn compact_ranges(ranges: impl Iterator<Item = LineRange>, line_count: u32) -> Vec<Range<u32>> {
    let mut compact: Vec<Range<u32>> = Vec::new();
    for range in ranges {
        let start = range
            .start_line
            .saturating_sub(COMPACT_CONTEXT_LINES)
            .max(1);
        let end = if range.is_empty() {
            range.start_line.saturating_add(1)
        } else {
            range.end_line
        }
        .saturating_add(COMPACT_CONTEXT_LINES)
        .min(line_count.saturating_add(1));
        if start >= end {
            continue;
        }

        if let Some(previous) = compact.last_mut()
            && start <= previous.end
        {
            previous.end = previous.end.max(end);
            continue;
        }
        compact.push(start..end);
    }
    compact
}

fn source_line_in_ranges(content: ViewLineContent, ranges: &[Range<u32>]) -> bool {
    content
        .line()
        .is_some_and(|line| ranges.iter().any(|range| range.contains(&line)))
}

fn fold_marker_view_line() -> ViewLine {
    ViewLine {
        original: ViewLineContent::Filler,
        modified: ViewLineContent::Filler,
        kind: ViewLineType::Unchanged,
    }
}
