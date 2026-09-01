use align::{Alignment, DiffVersion};
use vscode_diff::{CharRange, DetailedLineRangeMapping, LineRange, LinesDiff, RangeMapping};

fn mapping(
    original: LineRange,
    modified: LineRange,
    inner_changes: Vec<RangeMapping>,
) -> LinesDiff {
    LinesDiff {
        changes: vec![DetailedLineRangeMapping {
            original,
            modified,
            inner_changes,
        }],
        moves: Vec::new(),
        hit_timeout: false,
    }
}

fn range(start_line: u32, start_col: u32, end_line: u32, end_col: u32) -> CharRange {
    CharRange {
        start_line,
        start_col,
        end_line,
        end_col,
    }
}

#[test]
fn a_pure_deletion_has_a_whole_line_character_decoration() {
    let original = ["keep", "removed", "keep"];
    let modified = ["keep", "keep"];
    let diff = mapping(
        LineRange {
            start_line: 2,
            end_line: 3,
        },
        LineRange {
            start_line: 2,
            end_line: 2,
        },
        Vec::new(),
    );
    let alignment = Alignment::new(diff, &original, &modified);

    let decoration = alignment.decorations(DiffVersion::Original, 2);
    assert!(decoration.line_background);
    assert!(decoration.gutter_background);
    assert_eq!(decoration.characters.len(), 1);
    assert_eq!(decoration.characters[0].bytes, 0..7);
    assert!(decoration.characters[0].fill_to_edge);
}

#[test]
fn a_filler_inside_a_replacement_does_not_make_the_other_line_whole() {
    let original = ["keep", "old", "left over", "keep"];
    let modified = ["keep", "new", "keep"];
    let diff = mapping(
        LineRange {
            start_line: 2,
            end_line: 4,
        },
        LineRange {
            start_line: 2,
            end_line: 3,
        },
        vec![RangeMapping {
            original: range(2, 1, 2, 4),
            modified: range(2, 1, 2, 4),
        }],
    );
    let alignment = Alignment::new(diff, &original, &modified);

    let decoration = alignment.decorations(DiffVersion::Original, 3);
    assert!(decoration.line_background);
    assert!(decoration.characters.is_empty());
}

#[test]
fn a_multiline_range_fills_each_crossed_line_break() {
    let original = ["abcd", "efgh"];
    let modified = ["ABCD", "EFGH"];
    let diff = mapping(
        LineRange {
            start_line: 1,
            end_line: 3,
        },
        LineRange {
            start_line: 1,
            end_line: 3,
        },
        vec![RangeMapping {
            original: range(1, 2, 2, 2),
            modified: range(1, 2, 2, 2),
        }],
    );
    let alignment = Alignment::new(diff, &original, &modified);

    let first = alignment.decorations(DiffVersion::Original, 1);
    assert_eq!(first.characters[0].bytes, 1..4);
    assert!(first.characters[0].fill_to_edge);
    let second = alignment.decorations(DiffVersion::Original, 2);
    assert_eq!(second.characters[0].bytes, 0..1);
    assert!(!second.characters[0].fill_to_edge);
}

#[test]
fn a_range_ending_at_the_line_break_fills_to_the_edge() {
    let original = ["abcd"];
    let modified = ["ABCD"];
    let diff = mapping(
        LineRange {
            start_line: 1,
            end_line: 2,
        },
        LineRange {
            start_line: 1,
            end_line: 2,
        },
        vec![RangeMapping {
            original: range(1, 2, 1, 5),
            modified: range(1, 2, 1, 5),
        }],
    );
    let alignment = Alignment::new(diff, &original, &modified);

    let decoration = alignment.decorations(DiffVersion::Original, 1);
    assert_eq!(decoration.characters[0].bytes, 1..4);
    assert!(decoration.characters[0].fill_to_edge);
}

#[test]
fn a_range_at_the_end_of_a_nonfinal_line_stays_finite() {
    let original = ["abcd", "keep"];
    let modified = ["ABCD", "keep"];
    let diff = mapping(
        LineRange {
            start_line: 1,
            end_line: 2,
        },
        LineRange {
            start_line: 1,
            end_line: 2,
        },
        vec![RangeMapping {
            original: range(1, 2, 1, 5),
            modified: range(1, 2, 1, 5),
        }],
    );
    let alignment = Alignment::new(diff, &original, &modified);

    let decoration = alignment.decorations(DiffVersion::Original, 1);
    assert!(!decoration.characters[0].fill_to_edge);
}

#[test]
fn a_paired_range_reaching_both_line_ends_includes_the_line_break() {
    let original = ["", "keep"];
    let modified = ["abc", "keep"];
    let diff = mapping(
        LineRange {
            start_line: 1,
            end_line: 2,
        },
        LineRange {
            start_line: 1,
            end_line: 2,
        },
        vec![RangeMapping {
            original: range(1, 1, 1, 1),
            modified: range(1, 1, 1, 4),
        }],
    );
    let alignment = Alignment::new(diff, &original, &modified);

    let decoration = alignment.decorations(DiffVersion::Original, 1);
    assert!(decoration.empty_markers.is_empty());
    assert!(decoration.characters[0].fill_to_edge);
}

#[test]
fn an_empty_inner_range_remains_a_marker() {
    let original = ["abc"];
    let modified = ["axbc"];
    let diff = mapping(
        LineRange {
            start_line: 1,
            end_line: 2,
        },
        LineRange {
            start_line: 1,
            end_line: 2,
        },
        vec![RangeMapping {
            original: range(1, 2, 1, 2),
            modified: range(1, 2, 1, 3),
        }],
    );
    let alignment = Alignment::new(diff, &original, &modified);

    let decoration = alignment.decorations(DiffVersion::Original, 1);
    assert_eq!(decoration.empty_markers, [1]);
    assert!(decoration.characters.is_empty());
}

#[test]
fn an_empty_marker_starting_outside_its_mapping_is_hidden() {
    let original = ["changed", "outside"];
    let modified = ["CHANGED", "outside"];
    let diff = mapping(
        LineRange {
            start_line: 1,
            end_line: 2,
        },
        LineRange {
            start_line: 1,
            end_line: 2,
        },
        vec![RangeMapping {
            original: range(2, 1, 2, 1),
            modified: range(2, 1, 2, 1),
        }],
    );
    let alignment = Alignment::new(diff, &original, &modified);

    assert!(
        alignment
            .decorations(DiffVersion::Original, 2)
            .empty_markers
            .is_empty()
    );
}
