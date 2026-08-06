//! Exact view lines for hand-built diffs.
//!
//! The fixture and property tests check that each column *reads back* as its
//! file, which a wrong pairing can still satisfy: swapping the two fillers
//! inside a changed block leaves both columns intact and every invariant true.
//! These pin which line sits opposite which.

use align::{Alignment, Malformed, Slot, ViewLineType};
use file_types::DiffType;
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
        .view_lines(DiffType::SideBySide)
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
    assert_eq!(alignment.view_line_count(DiffType::SideBySide), 4);
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

// --- where the fillers go inside a change ---------------------------------
//
// A change is not one block with its fillers at the bottom. Where the engine
// matched text across it, the lines carrying that text are pulled level and
// the fillers go around them. These pin that, because the columns read back
// as their files either way and every other invariant stays true.

#[test]
fn a_line_that_survived_a_rewrite_sits_beside_itself() {
    // Real text, from the change that showed this up: a doc comment loses two
    // lines and rewrites the rest, and the engine reports the whole thing as
    // one change with inner detail. Without splitting on that detail the two
    // fillers land at the bottom, and every line of the comment sits opposite
    // the wrong one.
    let original = split(
        "impl std::fmt::Debug for Highlighted {\n\
         \x20   /// How far it has got, not every span it found.\n\
         \x20   ///\n\
         \x20   /// Written out rather than derived because the derived form is tens of\n\
         \x20   /// thousands of byte ranges, which no failing test is easier to read for.\n\
         \x20   fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {",
    );
    let modified = split(
        "impl std::fmt::Debug for Highlighted {\n\
         \x20   /// Written out rather than derived because `Reading` is a grammar's\n\
         \x20   /// context stack, which no failing test is easier to read for.\n\
         \x20   fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {",
    );
    let alignment = Alignment::new(compute(&original, &modified), &original, &modified);

    assert_eq!(
        table(&alignment),
        vec![
            (Some(1), Some(1), Unchanged),
            (Some(2), None, Deleted), // the two lines that went
            (Some(3), None, Deleted),
            (Some(4), Some(2), Modified), // "Written out rather than derived"
            (Some(5), Some(3), Modified), // "which no failing test is easier"
            (Some(6), Some(4), Unchanged),
        ],
        "the rewritten lines pair with what they became"
    );
}

#[test]
fn a_block_with_nothing_matching_puts_its_fillers_first() {
    // Built by hand: this engine always reports inner detail when both sides
    // have text, so nothing it produces reaches this branch. The plugin has
    // the rule, a slower engine that gave up could produce the shape, and it
    // is ten lines — so it is here and pinned rather than guessed at later.
    let original = split("one\ntwo\nthree");
    let modified = split("one\nalpha\nbeta\ngamma\nthree");
    let blunt = LinesDiff {
        changes: vec![DetailedLineRangeMapping {
            original: LineRange {
                start_line: 2,
                end_line: 3,
            },
            modified: LineRange {
                start_line: 2,
                end_line: 5,
            },
            inner_changes: vec![],
        }],
        moves: vec![],
        hit_timeout: false,
    };
    let alignment = Alignment::new(blunt, &original, &modified);

    assert_eq!(
        table(&alignment),
        vec![
            (Some(1), Some(1), Unchanged),
            (None, Some(2), Inserted), // the fillers open the block
            (None, Some(3), Inserted),
            (Some(2), Some(4), Modified),
            (Some(3), Some(5), Unchanged),
        ]
    );
}

#[test]
fn the_view_line_count_matches_the_walk() {
    // Splitting a change can make it taller than either side, so the count
    // cannot be derived from the line totals and must come from the same walk.
    for (original, modified) in [
        (
            "impl std::fmt::Debug for Highlighted {\n    /// How far it has got.\n    ///\n    /// Written out rather than derived because the derived form is tens of\n    /// thousands of byte ranges, which no failing test is easier to read for.\n    fn fmt(&self) {",
            "impl std::fmt::Debug for Highlighted {\n    /// Written out rather than derived because `Reading` is a grammar's\n    /// context stack, which no failing test is easier to read for.\n    fn fmt(&self) {",
        ),
        ("head\nkeep\ntail one", "head A\nhead B\nkeep\ntail two"),
        ("one\ntwo\nthree", "one\nalpha\nbeta\ngamma\nthree"),
        ("a\nb\nc\nd", "a\nb\nc\nd"),
        ("x", "p\nq\nr\ns\nt"),
    ] {
        let original = split(original);
        let modified = split(modified);
        let alignment = Alignment::new(compute(&original, &modified), &original, &modified);
        for layout in [DiffType::SideBySide, DiffType::Inline] {
            assert_eq!(
                alignment.view_line_count(layout) as usize,
                alignment.view_lines(layout).count(),
                "{layout:?} on {original:?} -> {modified:?}"
            );
        }
    }
}
