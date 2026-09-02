//! Normalization VS Code applies between its diff provider and `DiffState`.
//!
//! These are `normalizeDocumentDiff` and `normalizeRangeMapping` from
//! `diffEditorViewModel.ts`: paired ranges beginning at column one and reaching
//! both line ends include the line break.

use diff_types::{CharRange, LinesDiff, RangeMapping};

pub(crate) fn normalize_document_diff(
    diff: &mut LinesDiff,
    original: &[String],
    modified: &[String],
) {
    for change in &mut diff.changes {
        for inner in &mut change.inner_changes {
            normalize_range_mapping(inner, original, modified);
        }
    }
}

fn normalize_range_mapping(mapping: &mut RangeMapping, original: &[String], modified: &[String]) {
    if should_extend(&mapping.original, original, &mapping.modified, modified) {
        extend(&mut mapping.original);
        extend(&mut mapping.modified);
    }
}

fn should_extend(
    original_range: &CharRange,
    original: &[String],
    modified_range: &CharRange,
    modified: &[String],
) -> bool {
    original_range.start_col == 1
        && modified_range.start_col == 1
        && (original_range.end_col != 1 || modified_range.end_col != 1)
        && ends_at_line_end(original_range, original)
        && ends_at_line_end(modified_range, modified)
        && original_range.end_line < original.len() as u32
        && modified_range.end_line < modified.len() as u32
}

fn ends_at_line_end(range: &CharRange, lines: &[String]) -> bool {
    lines
        .get(range.end_line.saturating_sub(1) as usize)
        .is_some_and(|line| range.end_col as usize == line.encode_utf16().count() + 1)
}

fn extend(range: &mut CharRange) {
    range.end_line += 1;
    range.end_col = 1;
}
