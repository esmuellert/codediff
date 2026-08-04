//! What the two layouts must agree about.
//!
//! Inline and side by side lay a diff out differently on purpose, so almost
//! nothing about them is comparable — line counts differ, and line *n* of one is
//! not line *n* of the other. Exactly one thing has to hold, and it is the
//! thing that makes a second layout trustworthy: **each version reads back
//! as its own file, in order, in either layout.**
//!
//! That single property catches essentially every plausible mistake in a
//! walk — a line emitted twice, one skipped, deletions and insertions
//! interleaved wrongly, an unchanged run mispaired, a change's lines counted
//! against the wrong side.

use align::{Alignment, DiffLayout, DiffVersion, ViewLineType};
use vscode_diff::{LinesDiff, Options};

fn split(text: &str) -> Vec<&str> {
    text.split('\n').collect()
}

fn compute(original: &[&str], modified: &[&str]) -> LinesDiff {
    vscode_diff::compute(original, modified, &Options::default().with_moves())
        .expect("these inputs are tiny")
}

fn aligned(original: &str, modified: &str) -> Alignment {
    let original = split(original);
    let modified = split(modified);
    Alignment::new(compute(&original, &modified), &original, &modified)
}

/// Every line of one version, in the order the lines put them.
fn read_back(alignment: &Alignment, layout: DiffLayout, version: DiffVersion) -> Vec<&str> {
    alignment
        .view_lines(layout)
        .filter_map(|line| match version {
            DiffVersion::Original => line.original.line(),
            DiffVersion::Modified => line.modified.line(),
        })
        .map(|n| alignment.line(version, n).expect("the line exists"))
        .collect()
}

macro_rules! vendored {
    ($($name:ident),* $(,)?) => {
        [$((
            stringify!($name),
            include_str!(concat!("../../../vendor/test-pairs/", stringify!($name), "/original.txt")),
            include_str!(concat!("../../../vendor/test-pairs/", stringify!($name), "/modified.txt")),
        )),*]
    };
}

/// The twelve vendored pairs, plus edge shapes they do not cover.
///
/// The vendored ones were crafted upstream to stress move detection, which
/// produces the most awkward change ranges; the hand-written ones cover the
/// boundaries — empty files, a change touching the very first or last line.
fn pairs() -> Vec<(&'static str, &'static str, &'static str)> {
    let mut pairs: Vec<_> = vendored![
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
    ]
    .to_vec();
    pairs.extend([
        ("nothing changed", "one\ntwo\nthree", "one\ntwo\nthree"),
        ("one line edited", "one\ntwo\nthree", "one\nTWO\nthree"),
        ("pure insertion", "one\ntwo", "one\ninserted\ntwo"),
        ("pure deletion", "one\ngone\ntwo", "one\ntwo"),
        ("uneven replacement", "a\nb\nc\nd", "a\nX\nY\nZ\nW\nd"),
        ("everything replaced", "a\nb\nc", "x\ny\nz"),
        ("change at the very start", "a\nb\nc", "X\nb\nc"),
        ("change at the very end", "a\nb\nc", "a\nb\nZ"),
        ("empty original", "", "a\nb\nc"),
        ("empty modified", "a\nb\nc", ""),
        ("both empty", "", ""),
        ("many small changes", "a\nb\nc\nd\ne\nf", "a\nB\nc\nD\ne\nF"),
    ]);
    pairs
}

#[test]
fn both_layouts_read_back_as_the_same_two_files() {
    for (name, before, after) in pairs() {
        let alignment = aligned(before, after);
        for version in [DiffVersion::Original, DiffVersion::Modified] {
            let paired = read_back(&alignment, DiffLayout::SideBySide, version);
            let inline = read_back(&alignment, DiffLayout::Inline, version);
            assert_eq!(paired, inline, "{name}, {version:?}");
            assert_eq!(
                paired,
                alignment.lines(version),
                "{name}, {version:?}: not the file itself"
            );
        }
    }
}

#[test]
fn a_counted_layout_is_as_tall_as_it_walks() {
    for (name, before, after) in pairs() {
        let alignment = aligned(before, after);
        for layout in [DiffLayout::SideBySide, DiffLayout::Inline] {
            assert_eq!(
                alignment.view_line_count(layout) as usize,
                alignment.view_lines(layout).count(),
                "{name}, {layout:?}"
            );
        }
    }
}

#[test]
fn inline_is_never_shorter_than_side_by_side() {
    // A change costs the sum of its sides inline and the taller of them side
    // by side, and the two are equal only when one side is empty.
    for (name, before, after) in pairs() {
        let alignment = aligned(before, after);
        assert!(
            alignment.view_line_count(DiffLayout::Inline)
                >= alignment.view_line_count(DiffLayout::SideBySide),
            "{name}"
        );
    }
}

#[test]
fn no_inline_row_holds_both_versions_unless_they_agree() {
    // The defining property of the layout: a line belongs to one version, and
    // the only exception is an unchanged line, which both versions share.
    for (name, before, after) in pairs() {
        let alignment = aligned(before, after);
        for line in alignment.view_lines(DiffLayout::Inline) {
            if line.original.line().is_some() && line.modified.line().is_some() {
                assert_eq!(line.kind, ViewLineType::Unchanged, "{name}: {line:?}");
            }
        }
    }
}

#[test]
fn inline_shows_what_was_there_before_what_replaced_it() {
    // Reading the other order would describe an edit backwards.
    let alignment = aligned("a\nb\nc", "a\nX\nc");
    let kinds: Vec<_> = alignment
        .view_lines(DiffLayout::Inline)
        .map(|l| l.kind)
        .collect();
    assert_eq!(
        kinds,
        vec![
            ViewLineType::Unchanged,
            ViewLineType::Deleted,
            ViewLineType::Inserted,
            ViewLineType::Unchanged
        ]
    );
}

#[test]
fn a_view_line_maps_to_a_file_line_and_back_within_its_own_layout() {
    for (name, before, after) in pairs() {
        let alignment = aligned(before, after);
        for layout in [DiffLayout::SideBySide, DiffLayout::Inline] {
            for view_line in 0..alignment.view_line_count(layout) {
                let Some((version, line)) = alignment.line_at(layout, view_line) else {
                    continue;
                };
                assert_eq!(
                    alignment.view_line_at(layout, version, line),
                    Some(view_line),
                    "{name}, {layout:?}: view line {view_line} lost its way back"
                );
            }
        }
    }
}

#[test]
fn a_file_line_keeps_its_place_when_the_layout_changes() {
    // What the layout toggle relies on: the line number is meaningless in the
    // other layout, but the line it shows is not.
    for (name, before, after) in pairs() {
        let alignment = aligned(before, after);
        for view_line in 0..alignment.view_line_count(DiffLayout::SideBySide) {
            let Some((version, line)) = alignment.line_at(DiffLayout::SideBySide, view_line) else {
                continue;
            };
            let moved = alignment
                .view_line_at(DiffLayout::Inline, version, line)
                .unwrap_or_else(|| panic!("{name}: line {line} has no inline line"));
            assert_eq!(
                alignment.line_at(DiffLayout::Inline, moved),
                Some((version, line)),
                "{name}: landed somewhere else"
            );
        }
    }
}

#[test]
fn blocks_cover_every_changed_row_and_nothing_else() {
    for (name, before, after) in pairs() {
        let alignment = aligned(before, after);
        for layout in [DiffLayout::SideBySide, DiffLayout::Inline] {
            let blocks = alignment.blocks(layout);
            let covered: Vec<u32> = blocks.iter().flat_map(|b| b.clone()).collect();
            let changed: Vec<u32> = alignment
                .view_lines(layout)
                .enumerate()
                .filter(|(_, line)| line.kind != ViewLineType::Unchanged)
                .map(|(i, _)| i as u32)
                .collect();
            assert_eq!(covered, changed, "{name}, {layout:?}");
            // Adjacent lines belong to one block, or navigation would stop
            // twice inside a single edit.
            for pair in blocks.windows(2) {
                assert!(
                    pair[1].start > pair[0].end,
                    "{name}, {layout:?}: {blocks:?}"
                );
            }
        }
    }
}
