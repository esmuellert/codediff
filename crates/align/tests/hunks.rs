//! Hunks: how changes group, and what keeps a hunk's identity across a refresh.

use align::{Alignment, RowKind, Slot};
use vscode_diff::{LinesDiff, Options};

fn split(text: &str) -> Vec<&str> {
    text.split('\n').collect()
}

fn compute(original: &[&str], modified: &[&str]) -> LinesDiff {
    vscode_diff::compute(original, modified, &Options::default().with_moves())
        .expect("these inputs are tiny")
}

#[test]
fn a_hunks_identity_follows_its_text_and_not_its_position() {
    // Pushing a hunk down the file must not mark it unread again.
    let original = split("a\nb\nc");
    let modified = split("a\nB\nc");
    let diff = compute(&original, &modified);
    let here = Alignment::new(diff.clone(), &original, &modified);

    let moved_original = split("new\nlines\nhere\na\nb\nc");
    let moved_modified = split("new\nlines\nhere\na\nB\nc");
    let moved_diff = compute(&moved_original, &moved_modified);
    let there = Alignment::new(moved_diff.clone(), &moved_original, &moved_modified);

    assert_eq!(
        here.hunks()[0].id,
        there.hunks()[0].id,
        "the same edit further down the file should keep its identity"
    );
}

#[test]
fn editing_a_hunk_changes_its_identity() {
    let original = split("a\nb\nc");
    let first = split("a\nB\nc");
    let second = split("a\nDIFFERENT\nc");

    let first_diff = compute(&original, &first);
    let second_diff = compute(&original, &second);

    assert_ne!(
        Alignment::new(first_diff.clone(), &original, &first).hunks()[0].id,
        Alignment::new(second_diff.clone(), &original, &second).hunks()[0].id,
        "a different edit must not inherit the old identity"
    );
}

#[test]
fn the_line_separator_stops_neighbours_hashing_alike() {
    // Without a separator between lines, ["ab", "c"] and ["a", "bc"] would
    // hash the same and two unrelated hunks would share a review mark.
    let original = split("x\nab\nc\ny");
    let first = split("x\nQ\nQ\ny");
    let second = split("x\nQQ\n\ny");

    let a = compute(&original, &first);
    let b = compute(&original, &second);
    assert_ne!(
        Alignment::new(a.clone(), &original, &first).hunks()[0].id,
        Alignment::new(b.clone(), &original, &second).hunks()[0].id
    );
}

#[test]
fn nearby_changes_join_and_distant_ones_do_not() {
    // Two edits with one unchanged line between them read as one edit.
    let original = split("a\nb\nc\nd\ne\nf\ng\nh\ni\nj\nk\nl");
    let near = split("A\nb\nC\nd\ne\nf\ng\nh\ni\nj\nk\nl");
    let diff = compute(&original, &near);
    let alignment = Alignment::with_options(diff.clone(), &original, &near, 4, 3);
    assert_eq!(
        alignment.hunks().len(),
        1,
        "changes one line apart belong to the same hunk"
    );

    // The same two edits far apart read as two.
    let far = split("A\nb\nc\nd\ne\nf\ng\nh\ni\nj\nk\nL");
    let diff = compute(&original, &far);
    let alignment = Alignment::with_options(diff.clone(), &original, &far, 4, 3);
    assert_eq!(alignment.hunks().len(), 2);
}

#[test]
fn a_wider_context_merges_more() {
    let original = split("a\nb\nc\nd\ne\nf\ng\nh");
    let modified = split("A\nb\nc\nd\ne\nf\ng\nH");
    let diff = compute(&original, &modified);

    assert_eq!(
        Alignment::with_options(diff.clone(), &original, &modified, 4, 3)
            .hunks()
            .len(),
        2
    );
    assert_eq!(
        Alignment::with_options(diff.clone(), &original, &modified, 4, 99)
            .hunks()
            .len(),
        1
    );
}

#[test]
fn a_file_with_no_changes_has_no_hunks_and_only_unchanged_rows() {
    let text = split("one\ntwo\nthree");
    let diff = compute(&text, &text);
    let alignment = Alignment::new(diff.clone(), &text, &text);

    assert_eq!(alignment.row_count(), 3);
    assert!(alignment.hunks().is_empty());
    for (i, row) in alignment.rows().enumerate() {
        assert_eq!(row.kind, RowKind::Unchanged);
        assert_eq!(row.original, Slot::Line(i as u32 + 1));
        assert_eq!(row.modified, Slot::Line(i as u32 + 1));
    }
}

#[test]
fn an_empty_file_is_one_empty_line() {
    // The engine models an empty file that way, and `vscode-diff` normalises to
    // it before computing, so an `Alignment` that did not would hold a file its
    // own diff refers to lines of. Found by proptest.
    let empty: Vec<&str> = Vec::new();
    let added = split("hello");
    let diff = compute(&empty, &added);
    let alignment = Alignment::new(diff.clone(), &empty, &added);

    assert_eq!(alignment.lines(align::Side::Original), &[""]);
    assert_eq!(alignment.row_count(), 1);
    assert_eq!(alignment.rows().count(), 1);
}

#[test]
fn hunk_change_ranges_partition_the_changes() {
    let original = split("a\nb\nc\nd\ne\nf\ng\nh\ni\nj\nk\nl");
    let modified = split("A\nb\nc\nd\ne\nf\ng\nh\ni\nj\nk\nL");
    let diff = compute(&original, &modified);
    let alignment = Alignment::new(diff.clone(), &original, &modified);

    let mut next = 0;
    for hunk in alignment.hunks() {
        assert_eq!(hunk.changes.start, next, "hunks must not overlap or skip");
        next = hunk.changes.end;
    }
    assert_eq!(next, diff.changes.len(), "hunks must cover every change");
}

#[test]
fn two_identical_edits_in_one_file_get_different_identities() {
    // Same edit twice. Content alone hashes alike, so marking one reviewed
    // would mark the other.
    let original = split("a\nX\nb\nc\nd\ne\nf\ng\na\nX\nb");
    let modified = split("a\nY\nb\nc\nd\ne\nf\ng\na\nY\nb");
    let diff = compute(&original, &modified);
    let alignment = Alignment::new(diff.clone(), &original, &modified);

    assert_eq!(alignment.hunks().len(), 2);
    let (first, second) = (alignment.hunks()[0].id, alignment.hunks()[1].id);
    assert_ne!(
        first, second,
        "identical hunks must still be distinguishable"
    );
    assert_eq!(
        alignment.hunk(first).map(|h| h.original.start_line),
        Some(2)
    );
    assert_eq!(
        alignment.hunk(second).map(|h| h.original.start_line),
        Some(10)
    );
}
