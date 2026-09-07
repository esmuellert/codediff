//! Tests character ranges that cross line boundaries.

use align::{Alignment, DiffVersion};
use vscode_diff::{
    CharRange, DetailedLineRangeMapping, LineRange, LinesDiff, Options, RangeMapping,
};

fn split(text: &str) -> Vec<&str> {
    text.split('\n').collect()
}

fn compute(original: &[&str], modified: &[&str]) -> LinesDiff {
    vscode_diff::compute(original, modified, &Options::default()).expect("these inputs are tiny")
}

#[test]
fn a_span_crossing_a_line_boundary_covers_each_line_correctly() {
    // A range can span multiple lines.
    let original = split("keep\nalpha\nbravo\nkeep");
    let modified = split("keep\nALPHA\nBRAVO\nkeep");
    let crossing = LinesDiff {
        changes: vec![DetailedLineRangeMapping {
            original: LineRange {
                start_line: 2,
                end_line: 4,
            },
            modified: LineRange {
                start_line: 2,
                end_line: 4,
            },
            inner_changes: vec![RangeMapping {
                original: CharRange {
                    start_line: 2,
                    start_col: 1,
                    end_line: 3,
                    end_col: 6,
                },
                modified: CharRange {
                    start_line: 2,
                    start_col: 1,
                    end_line: 3,
                    end_col: 6,
                },
            }],
        }],
        moves: vec![],
        hit_timeout: false,
    };
    let alignment = Alignment::new(crossing.clone(), &original, &modified);

    // Line 2 from column 1 to its end, line 3 from its start to column 6.
    let second = alignment.spans(DiffVersion::Original, 2);
    assert_eq!(second.len(), 1);
    assert_eq!(
        &original[1][second[0].bytes.start as usize..second[0].bytes.end as usize],
        "alpha"
    );

    let third = alignment.spans(DiffVersion::Original, 3);
    assert_eq!(third.len(), 1);
    assert_eq!(
        &original[2][third[0].bytes.start as usize..third[0].bytes.end as usize],
        "bravo"
    );
}

#[test]
fn a_span_ending_at_column_one_contributes_no_final_line() {
    // An end column of 1 contributes no bytes on that line.
    let text = split("keep\nalpha\nkeep");
    let diff = LinesDiff {
        changes: vec![DetailedLineRangeMapping {
            original: LineRange {
                start_line: 2,
                end_line: 3,
            },
            modified: LineRange {
                start_line: 2,
                end_line: 3,
            },
            inner_changes: vec![RangeMapping {
                original: CharRange {
                    start_line: 2,
                    start_col: 1,
                    end_line: 3,
                    end_col: 1,
                },
                modified: CharRange {
                    start_line: 2,
                    start_col: 1,
                    end_line: 3,
                    end_col: 1,
                },
            }],
        }],
        moves: vec![],
        hit_timeout: false,
    };
    let alignment = Alignment::new(diff.clone(), &text, &text);

    assert_eq!(alignment.spans(DiffVersion::Original, 2).len(), 1);
    assert!(alignment.spans(DiffVersion::Original, 3).is_empty());
}

#[test]
fn a_real_edit_reports_the_characters_that_changed() {
    // A real edit must produce a non-empty span.
    let original = split("let timeout = 30;");
    let modified = split("let timeout = 45;");
    let diff = compute(&original, &modified);
    let alignment = Alignment::new(diff.clone(), &original, &modified);

    let spans = alignment.spans(DiffVersion::Modified, 1);
    assert!(!spans.is_empty(), "an edited line must report a span");
    let covered: String = spans
        .iter()
        .map(|s| &modified[0][s.bytes.start as usize..s.bytes.end as usize])
        .collect();
    assert!(
        covered.contains("45"),
        "the span should cover the new text, got {covered:?}"
    );
}
