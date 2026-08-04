//! Exact view lines for hand-built diffs.
//!
//! The fixture and property tests check that each column *reads back* as its
//! file, which a wrong pairing can still satisfy: swapping the two fillers
//! inside a changed block leaves both columns intact and every invariant true.
//! These pin which line sits opposite which.

use align::{Alignment, DiffLayout, Malformed, Slot, ViewLineType};
use vscode_diff::{DetailedLineRangeMapping, LineRange, LinesDiff, Options};

fn split(text: &str) -> Vec<&str> {
    text.split('\n').collect()
}

fn compute(original: &[&str], modified: &[&str]) -> LinesDiff {
    vscode_diff::compute(original, modified, &Options::default().with_moves())
        .expect("these inputs are tiny")
}

/// `(original, modified, kind)` for every line, as a readable table.
fn table(alignment: &Alignment) -> Vec<(Option<u32>, Option<u32>, ViewLineType)> {
    alignment
        .view_lines(DiffLayout::SideBySide)
        .map(|r| (r.original.line(), r.modified.line(), r.kind))
        .collect()
}

use ViewLineType::{Deleted, Inserted, Modified, Unchanged};

#[test]
fn a_deletion_and_an_insertion_land_on_the_right_rows() {
    let original = split("one\ntwo\nthree\nfour\nfive");
    let modified = split("one\nthree\nfour\nNEW\nfive");
    let diff = compute(&original, &modified);
    let alignment = Alignment::new(diff.clone(), &original, &modified);

    assert_eq!(
        table(&alignment),
        vec![
            (Some(1), Some(1), Unchanged),
            (Some(2), None, Deleted), // "two" removed
            (Some(3), Some(2), Unchanged),
            (Some(4), Some(3), Unchanged),
            (None, Some(4), Inserted), // "NEW" added
            (Some(5), Some(5), Unchanged),
        ]
    );
}

#[test]
fn a_change_taller_on_one_side_puts_its_fillers_last() {
    // Three original lines become one. The first line pairs, and the two extra
    // original lines fall opposite fillers *below* it, not interleaved.
    let original = split("a\nb\nc\nd\nz");
    let modified = split("a\nQ\nz");
    let diff = compute(&original, &modified);
    let alignment = Alignment::new(diff.clone(), &original, &modified);

    assert_eq!(
        table(&alignment),
        vec![
            (Some(1), Some(1), Unchanged),
            (Some(2), Some(2), Modified),
            (Some(3), None, Deleted),
            (Some(4), None, Deleted),
            (Some(5), Some(3), Unchanged),
        ]
    );
}

#[test]
fn a_change_at_the_very_first_line() {
    let original = split("a\nb");
    let modified = split("Z\nb");
    let diff = compute(&original, &modified);
    let alignment = Alignment::new(diff.clone(), &original, &modified);

    assert_eq!(
        table(&alignment),
        vec![(Some(1), Some(1), Modified), (Some(2), Some(2), Unchanged)]
    );
}

#[test]
fn a_change_reaching_the_last_line() {
    let original = split("a\nb");
    let modified = split("a\nZ");
    let diff = compute(&original, &modified);
    let alignment = Alignment::new(diff.clone(), &original, &modified);

    assert_eq!(
        table(&alignment),
        vec![(Some(1), Some(1), Unchanged), (Some(2), Some(2), Modified)]
    );
}

#[test]
fn adjacent_changes_with_no_unchanged_run_between_them() {
    // Two changes touching each other must not lose or repeat a line.
    let diff = LinesDiff {
        changes: vec![
            DetailedLineRangeMapping {
                original: LineRange {
                    start_line: 2,
                    end_line: 3,
                },
                modified: LineRange {
                    start_line: 2,
                    end_line: 2,
                },
                inner_changes: vec![],
            },
            DetailedLineRangeMapping {
                original: LineRange {
                    start_line: 3,
                    end_line: 3,
                },
                modified: LineRange {
                    start_line: 2,
                    end_line: 3,
                },
                inner_changes: vec![],
            },
        ],
        moves: vec![],
        hit_timeout: false,
    };
    let original = split("a\nb\nc");
    let modified = split("a\nX\nc");
    let alignment = Alignment::new(diff.clone(), &original, &modified);

    assert_eq!(
        table(&alignment),
        vec![
            (Some(1), Some(1), Unchanged),
            (Some(2), None, Deleted),
            (None, Some(2), Inserted),
            (Some(3), Some(3), Unchanged),
        ]
    );
    assert_eq!(alignment.view_line_count(DiffLayout::SideBySide), 4);
}

#[test]
fn an_empty_diff_pairs_every_line() {
    let text = split("a\nb\nc");
    let diff = LinesDiff {
        changes: vec![],
        moves: vec![],
        hit_timeout: false,
    };
    let alignment = Alignment::new(diff.clone(), &text, &text);
    assert_eq!(
        table(&alignment),
        vec![
            (Some(1), Some(1), Unchanged),
            (Some(2), Some(2), Unchanged),
            (Some(3), Some(3), Unchanged),
        ]
    );
}

#[test]
fn a_non_monotonic_diff_is_refused_rather_than_duplicating_rows() {
    // Without this check, `lines()` walks backwards and emits lines 1 and 2
    // twice while `view_line_count()` keeps reporting 3.
    let text = split("a\nb\nc");
    let backwards = LinesDiff {
        changes: vec![
            DetailedLineRangeMapping {
                original: LineRange {
                    start_line: 2,
                    end_line: 3,
                },
                modified: LineRange {
                    start_line: 2,
                    end_line: 3,
                },
                inner_changes: vec![],
            },
            DetailedLineRangeMapping {
                original: LineRange {
                    start_line: 1,
                    end_line: 2,
                },
                modified: LineRange {
                    start_line: 1,
                    end_line: 2,
                },
                inner_changes: vec![],
            },
        ],
        moves: vec![],
        hit_timeout: false,
    };
    assert_eq!(
        Alignment::try_new(backwards.clone(), &text, &text).err(),
        Some(Malformed)
    );
}

#[test]
fn a_change_running_past_the_end_of_its_file_is_refused() {
    let text = split("a\nb");
    let too_far = LinesDiff {
        changes: vec![DetailedLineRangeMapping {
            original: LineRange {
                start_line: 1,
                end_line: 99,
            },
            modified: LineRange {
                start_line: 1,
                end_line: 99,
            },
            inner_changes: vec![],
        }],
        moves: vec![],
        hit_timeout: false,
    };
    assert_eq!(
        Alignment::try_new(too_far.clone(), &text, &text).err(),
        Some(Malformed)
    );
}

#[test]
fn slot_reports_filler_correctly() {
    assert!(Slot::Filler.is_filler());
    assert_eq!(Slot::Filler.line(), None);
    assert!(!Slot::Line(3).is_filler());
    assert_eq!(Slot::Line(3).line(), Some(3));
}
