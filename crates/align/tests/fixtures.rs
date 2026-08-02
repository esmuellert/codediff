//! The twelve vendored pairs, checked against the files they came from.
//!
//! The governing property: **read the left column top to bottom and you have
//! the original file; read the right and you have the modified one.** Fillers
//! contribute nothing. If that holds, the pairing is right — nothing else about
//! alignment can be wrong while it does.
//!
//! Content is pulled in with `include_str!`, so these tests do no IO and the
//! crate stays provably pure.

use align::{Alignment, RowKind, Side};
use vscode_diff::{LinesDiff, Options};

macro_rules! pairs {
    ($($name:ident),* $(,)?) => {
        &[$((
            stringify!($name),
            include_str!(concat!("../../../vendor/test-pairs/", stringify!($name), "/original.txt")),
            include_str!(concat!("../../../vendor/test-pairs/", stringify!($name), "/modified.txt")),
        )),*]
    };
}

/// Every pair in `vendor/test-pairs`. Most were crafted upstream to exercise
/// move detection, which is the hardest case for a pairing to get right.
const PAIRS: &[(&str, &str, &str)] = pairs![
    adjacent_move,
    block_moved_down,
    comprehensive_move,
    duplicate_not_move,
    empty_files,
    large_file_move,
    long_distance_move,
    moved_with_edit,
    multi_move,
    no_moves_control,
    simple_swap,
    single_line_move,
];

/// Splits the way the engine does — see `codediff debug align`.
fn split(text: &str) -> Vec<&str> {
    text.split('\n').collect()
}

fn compute(original: &[&str], modified: &[&str]) -> LinesDiff {
    vscode_diff::compute(original, modified, &Options::default().with_moves())
        .expect("the vendored pairs are well within every limit")
}

/// Runs a check over every vendored pair.
///
/// The alignment borrows the line vectors, so they have to outlive it here
/// rather than being returned.
fn for_each_pair(mut check: impl FnMut(&str, &Alignment)) {
    for (name, original_text, modified_text) in PAIRS {
        let original = split(original_text);
        let modified = split(modified_text);
        let diff = compute(&original, &modified);
        let alignment = Alignment::new(diff.clone(), &original, &modified);
        check(name, &alignment);
    }
}

#[test]
fn each_column_reads_back_as_the_file_it_came_from() {
    for_each_pair(|name, alignment| {
        let mut left = Vec::new();
        let mut right = Vec::new();
        for row in alignment.rows() {
            if let Some(n) = row.original.line() {
                left.push(alignment.line(Side::Original, n).expect("line exists"));
            }
            if let Some(n) = row.modified.line() {
                right.push(alignment.line(Side::Modified, n).expect("line exists"));
            }
        }

        assert_eq!(
            left,
            alignment.lines(Side::Original),
            "{name}: the left column is not the original file"
        );
        assert_eq!(
            right,
            alignment.lines(Side::Modified),
            "{name}: the right column is not the modified file"
        );
    });
}

#[test]
fn every_line_appears_exactly_once_and_in_order() {
    for_each_pair(|name, alignment| {
        let (mut last_original, mut last_modified) = (0, 0);
        for row in alignment.rows() {
            if let Some(n) = row.original.line() {
                assert_eq!(
                    n,
                    last_original + 1,
                    "{name}: original line {n} out of order"
                );
                last_original = n;
            }
            if let Some(n) = row.modified.line() {
                assert_eq!(
                    n,
                    last_modified + 1,
                    "{name}: modified line {n} out of order"
                );
                last_modified = n;
            }
        }
        assert_eq!(
            last_original as usize,
            alignment.lines(Side::Original).len(),
            "{name}: original truncated"
        );
        assert_eq!(
            last_modified as usize,
            alignment.lines(Side::Modified).len(),
            "{name}: modified truncated"
        );
    });
}

#[test]
fn no_row_is_blank_on_both_sides() {
    for_each_pair(|name, alignment| {
        for (i, row) in alignment.rows().enumerate() {
            assert!(
                !(row.original.is_filler() && row.modified.is_filler()),
                "{name}: row {i} shows nothing on either side"
            );
        }
    });
}

#[test]
fn the_row_count_matches_the_rows_produced() {
    for_each_pair(|name, alignment| {
        assert_eq!(
            alignment.row_count() as usize,
            alignment.rows().count(),
            "{name}: row_count disagrees with the rows"
        );
    });
}

#[test]
fn unchanged_rows_hold_identical_text() {
    for_each_pair(|name, alignment| {
        for row in alignment.rows() {
            if row.kind != RowKind::Unchanged {
                continue;
            }
            let (o, m) = row.both().expect("an unchanged row has both sides");
            assert_eq!(
                alignment.line(Side::Original, o),
                alignment.line(Side::Modified, m),
                "{name}: rows {o}/{m} are called unchanged but differ"
            );
        }
    });
}

#[test]
fn a_filler_never_sits_opposite_an_unchanged_line() {
    for_each_pair(|name, alignment| {
        for row in alignment.rows() {
            let has_filler = row.original.is_filler() || row.modified.is_filler();
            assert_eq!(
                has_filler,
                matches!(row.kind, RowKind::Deleted | RowKind::Inserted),
                "{name}: {row:?} disagrees with its own kind"
            );
        }
    });
}

#[test]
fn every_changed_line_belongs_to_exactly_one_hunk() {
    for_each_pair(|name, alignment| {
        for change in alignment.changes() {
            for line in change.original.start_line..change.original.end_line {
                let owners = alignment
                    .hunks()
                    .iter()
                    .filter(|h| line >= h.original.start_line && line < h.original.end_line)
                    .count();
                assert_eq!(
                    owners, 1,
                    "{name}: original line {line} is in {owners} hunks"
                );
            }
            for line in change.modified.start_line..change.modified.end_line {
                let owners = alignment
                    .hunks()
                    .iter()
                    .filter(|h| line >= h.modified.start_line && line < h.modified.end_line)
                    .count();
                assert_eq!(
                    owners, 1,
                    "{name}: modified line {line} is in {owners} hunks"
                );
            }
        }
    });
}

#[test]
fn character_spans_are_sliceable_and_non_empty() {
    for_each_pair(|name, alignment| {
        for side in [Side::Original, Side::Modified] {
            let lines = alignment.lines(side);
            for number in 1..=lines.len() as u32 {
                let text = alignment.line(side, number).expect("line exists");
                for span in alignment.spans(side, number) {
                    assert_eq!(
                        span.line, number,
                        "{name}: span reported for the wrong line"
                    );
                    assert!(span.bytes.start < span.bytes.end, "{name}: empty span");
                    assert!(
                        text.get(span.bytes.start as usize..span.bytes.end as usize)
                            .is_some(),
                        "{name}: line {number} span {:?} is not on character boundaries",
                        span.bytes
                    );
                }
            }
        }
    });
}

#[test]
fn moves_are_found_by_line_number() {
    let (_, original_text, modified_text) = PAIRS
        .iter()
        .find(|(name, _, _)| *name == "block_moved_down")
        .expect("the fixture is vendored");
    let original = split(original_text);
    let modified = split(modified_text);
    let diff = compute(&original, &modified);
    let alignment = Alignment::new(diff.clone(), &original, &modified);

    let moved = diff.moves.first().expect("this fixture has a move");
    assert!(
        alignment
            .moved(Side::Original, moved.original.start_line)
            .is_some()
    );
    assert!(
        alignment
            .moved(Side::Original, moved.original.end_line - 1)
            .is_some()
    );
    assert!(
        alignment
            .moved(Side::Original, moved.original.end_line)
            .is_none()
    );
    assert!(
        alignment
            .moved(Side::Modified, moved.modified.start_line)
            .is_some()
    );
}

/// The refusal in `rows.rs` only earns its place if the engine really does hold
/// to the shape it checks for.
#[test]
fn the_engine_never_produces_a_malformed_diff() {
    for (name, original_text, modified_text) in PAIRS {
        let original = split(original_text);
        let modified = split(modified_text);
        let diff = compute(&original, &modified);
        assert!(
            Alignment::try_new(diff.clone(), &original, &modified).is_ok(),
            "{name}: the engine produced a diff align refuses"
        );
    }
}
