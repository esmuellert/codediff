//! The twelve oracle pairs, checked against their source files.
//!
//! The governing property: **read the left column top to bottom and you have
//! the original file; read the right and you have the modified one.** Fillers
//! contribute nothing. If that holds, the pairing is right — nothing else about
//! alignment can be wrong while it does.
//!
//! Content is pulled in with `include_str!`, so these tests do no IO and the
//! crate stays provably pure.

use align::{Alignment, DiffVersion, ViewLineType};
use file_types::DiffType;
use vscode_diff::{LinesDiff, Options};

macro_rules! pairs {
    ($($name:ident),* $(,)?) => {
        &[$((
            stringify!($name),
            include_str!(concat!("../../../libvscode-diff/tests/oracle/", stringify!($name), "/original.txt")),
            include_str!(concat!("../../../libvscode-diff/tests/oracle/", stringify!($name), "/modified.txt")),
        )),*]
    };
}

/// Every pair in `libvscode-diff/tests/oracle`. Most exercise move detection,
/// which is the hardest case for a pairing to get right.
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
        .expect("the oracle pairs are well within every limit")
}

/// Runs a check over every oracle pair.
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
        for line in alignment.view_lines(DiffType::SideBySide) {
            if let Some(n) = line.original.line() {
                left.push(
                    alignment
                        .line(DiffVersion::Original, n)
                        .expect("line exists"),
                );
            }
            if let Some(n) = line.modified.line() {
                right.push(
                    alignment
                        .line(DiffVersion::Modified, n)
                        .expect("line exists"),
                );
            }
        }

        assert_eq!(
            left,
            alignment.lines(DiffVersion::Original),
            "{name}: the left column is not the original file"
        );
        assert_eq!(
            right,
            alignment.lines(DiffVersion::Modified),
            "{name}: the right column is not the modified file"
        );
    });
}

#[test]
fn every_line_appears_exactly_once_and_in_order() {
    for_each_pair(|name, alignment| {
        let (mut last_original, mut last_modified) = (0, 0);
        for line in alignment.view_lines(DiffType::SideBySide) {
            if let Some(n) = line.original.line() {
                assert_eq!(
                    n,
                    last_original + 1,
                    "{name}: original line {n} out of order"
                );
                last_original = n;
            }
            if let Some(n) = line.modified.line() {
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
            alignment.lines(DiffVersion::Original).len(),
            "{name}: original truncated"
        );
        assert_eq!(
            last_modified as usize,
            alignment.lines(DiffVersion::Modified).len(),
            "{name}: modified truncated"
        );
    });
}

#[test]
fn no_row_is_blank_on_both_sides() {
    for_each_pair(|name, alignment| {
        for (i, line) in alignment.view_lines(DiffType::SideBySide).enumerate() {
            assert!(
                !(line.original.is_filler() && line.modified.is_filler()),
                "{name}: line {i} shows nothing on either version"
            );
        }
    });
}

#[test]
fn the_row_count_matches_the_rows_produced() {
    for_each_pair(|name, alignment| {
        assert_eq!(
            alignment.view_line_count(DiffType::SideBySide) as usize,
            alignment.view_lines(DiffType::SideBySide).count(),
            "{name}: view_line_count disagrees with the lines"
        );
    });
}

#[test]
fn unchanged_rows_hold_identical_text() {
    for_each_pair(|name, alignment| {
        for line in alignment.view_lines(DiffType::SideBySide) {
            if line.kind != ViewLineType::Unchanged {
                continue;
            }
            let (o, m) = line.line_pair().expect("an unchanged line has both sides");
            assert_eq!(
                alignment.line(DiffVersion::Original, o),
                alignment.line(DiffVersion::Modified, m),
                "{name}: lines {o}/{m} are called unchanged but differ"
            );
        }
    });
}

#[test]
fn a_filler_never_sits_opposite_an_unchanged_line() {
    for_each_pair(|name, alignment| {
        for line in alignment.view_lines(DiffType::SideBySide) {
            let has_filler = line.original.is_filler() || line.modified.is_filler();
            assert_eq!(
                has_filler,
                matches!(line.kind, ViewLineType::Deleted | ViewLineType::Inserted),
                "{name}: {line:?} disagrees with its own kind"
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
        for version in [DiffVersion::Original, DiffVersion::Modified] {
            let lines = alignment.lines(version);
            for number in 1..=lines.len() as u32 {
                let text = alignment.line(version, number).expect("line exists");
                for span in alignment.spans(version, number) {
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
        .expect("the fixture is checked in");
    let original = split(original_text);
    let modified = split(modified_text);
    let diff = compute(&original, &modified);
    let alignment = Alignment::new(diff.clone(), &original, &modified);

    let moved = diff.moves.first().expect("this fixture has a move");
    assert!(
        alignment
            .moved(DiffVersion::Original, moved.original.start_line)
            .is_some()
    );
    assert!(
        alignment
            .moved(DiffVersion::Original, moved.original.end_line - 1)
            .is_some()
    );
    assert!(
        alignment
            .moved(DiffVersion::Original, moved.original.end_line)
            .is_none()
    );
    assert!(
        alignment
            .moved(DiffVersion::Modified, moved.modified.start_line)
            .is_some()
    );
}

/// The refusal in `lines.rs` only earns its place if the engine really does hold
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
