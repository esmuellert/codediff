//! End-to-end use of the public API.
//!
//! This file is an integration test, so it compiles against `vscode-diff` the
//! way any other crate would: only `pub` items are visible, and there is no
//! `unsafe`, no marshalling and no manual free. If these pass, Rust code can
//! use the C engine directly.

use vscode_diff::{Error, Options, Side, compute};

/// Splits source text the way a caller reading a file would.
fn lines(text: &str) -> Vec<&str> {
    text.lines().collect()
}

#[test]
fn diffs_a_realistic_edit() {
    let original = lines(
        "\
fn main() {
    let total = 1;
    println!(\"{total}\");
}",
    );
    let modified = lines(
        "\
fn main() {
    let total = 42;
    println!(\"total is {total}\");
    std::process::exit(0);
}",
    );

    let diff = compute(&original, &modified, &Options::default()).expect("diff should succeed");

    assert!(!diff.is_empty());
    assert!(!diff.hit_timeout);

    // Lines 2 and 3 were edited and line 4 added, so the change spans original
    // lines 2..4 and modified lines 2..5 — 1-based, end-exclusive.
    assert_eq!(diff.changes.len(), 1, "{:?}", diff.changes);
    let change = &diff.changes[0];
    assert_eq!((change.original.start, change.original.end), (2, 4));
    assert_eq!((change.modified.start, change.modified.end), (2, 5));
    assert_eq!(change.original.len(), 2);
    assert_eq!(change.modified.len(), 3);

    assert!(
        !change.inner.is_empty(),
        "an edit within lines should carry character-level detail"
    );
}

#[test]
fn identical_text_produces_an_empty_diff() {
    let text = lines("alpha\nbeta\ngamma");
    let diff = compute(&text, &text, &Options::default()).unwrap();
    assert!(diff.is_empty());
    assert!(diff.changes.is_empty());
    assert!(diff.moves.is_empty());
}

#[test]
fn insertions_and_deletions_are_distinguishable() {
    let base = lines("alpha\ngamma");
    let with_extra = lines("alpha\nbeta\ngamma");

    let inserted = compute(&base, &with_extra, &Options::default()).unwrap();
    assert_eq!(inserted.changes.len(), 1);
    assert!(inserted.changes[0].is_insertion());
    assert!(!inserted.changes[0].is_deletion());
    assert!(inserted.changes[0].original.is_empty());

    let deleted = compute(&with_extra, &base, &Options::default()).unwrap();
    assert_eq!(deleted.changes.len(), 1);
    assert!(deleted.changes[0].is_deletion());
    assert!(!deleted.changes[0].is_insertion());
    assert!(deleted.changes[0].modified.is_empty());
}

#[test]
fn character_level_detail_locates_the_edit_within_a_line() {
    let original = ["let timeout = 30;"];
    let modified = ["let timeout = 60;"];

    let diff = compute(&original, &modified, &Options::default()).unwrap();
    let inner = &diff.changes[0].inner;
    assert!(!inner.is_empty());

    let first = inner[0];
    assert_eq!(first.original.start_line, 1);
    assert!(
        first.original.start_col > 1,
        "the shared prefix should be excluded, got {first:?}"
    );
    assert!(first.original.end_col > first.original.start_col);
}

#[test]
fn move_detection_is_opt_in() {
    let original = lines("aaa\nbbb\nccc\nddd\neee\nfff\nmoved1\nmoved2\nmoved3");
    let modified = lines("moved1\nmoved2\nmoved3\naaa\nbbb\nccc\nddd\neee\nfff");

    let without = compute(&original, &modified, &Options::default()).unwrap();
    assert!(without.moves.is_empty());

    let with = compute(&original, &modified, &Options::default().with_moves()).unwrap();
    assert!(
        !with.moves.is_empty(),
        "a relocated block should be reported when moves are requested"
    );
}

#[test]
fn whitespace_only_changes_can_be_ignored() {
    let original = ["value = 1;"];
    let modified = ["   value = 1;   "];

    let strict = compute(&original, &modified, &Options::default()).unwrap();
    assert!(
        !strict.is_empty(),
        "indentation differs, so this is a change"
    );

    let lenient = compute(
        &original,
        &modified,
        &Options::default().ignoring_trim_whitespace(),
    )
    .unwrap();
    assert!(
        lenient.is_empty(),
        "leading and trailing whitespace should be ignorable, got {:?}",
        lenient.changes
    );
}

#[test]
fn an_entirely_new_file_is_reported_as_added() {
    // The regression this guards: the engine models an empty file as a single
    // empty line, and handing it zero lines instead silently yields no changes.
    let added = compute(&[], &lines("alpha\nbeta\ngamma"), &Options::default()).unwrap();

    assert!(!added.is_empty(), "a new file's content is all added");
    assert_eq!(added.changes.len(), 1);
    assert_eq!(added.changes[0].modified.len(), 3);
}

#[test]
fn non_ascii_content_round_trips() {
    let original = lines("日本語テキスト\nemoji 🎉 here\ncafé");
    let modified = lines("日本語テキスト\nemoji 🚀 here\ncafé");

    let diff = compute(&original, &modified, &Options::default()).unwrap();
    assert_eq!(diff.changes.len(), 1, "{:?}", diff.changes);
    assert_eq!(
        (diff.changes[0].modified.start, diff.changes[0].modified.end),
        (2, 3)
    );
}

#[test]
fn a_large_file_diffs_within_the_time_budget() {
    let original: Vec<String> = (0..5_000).map(|i| format!("line {i} of content")).collect();
    let mut modified = original.clone();
    modified[2_500] = "line 2500 was edited".to_owned();
    modified.insert(4_000, "an inserted line".to_owned());

    let original: Vec<&str> = original.iter().map(String::as_str).collect();
    let modified: Vec<&str> = modified.iter().map(String::as_str).collect();

    let diff = compute(&original, &modified, &Options::default()).unwrap();

    assert!(!diff.hit_timeout, "5000 lines should be well within budget");
    assert_eq!(diff.changes.len(), 2, "{:?}", diff.changes);
    assert_eq!(diff.changes[0].original.start, 2_501);
    assert!(diff.changes[1].is_insertion());
}

#[test]
fn binary_content_is_rejected_rather_than_truncated() {
    let err = compute(&["ok"], &["has\0nul"], &Options::default()).unwrap_err();
    assert_eq!(
        err,
        Error::InteriorNul {
            side: Side::Modified,
            line: 1
        }
    );
    assert!(err.to_string().contains("NUL"));
}

#[test]
fn a_diff_outlives_the_call_and_can_cross_threads() {
    // Nothing in the result borrows from C, so it is an ordinary owned value.
    let diff = {
        let original = lines("alpha\nbeta");
        let modified = lines("alpha\nBETA");
        compute(&original, &modified, &Options::default()).unwrap()
    };

    let handle = std::thread::spawn(move || diff.changes.len());
    assert_eq!(handle.join().unwrap(), 1);
}

#[test]
fn repeated_use_is_stable() {
    let original = lines("alpha\nbeta\ngamma");
    for i in 0..500 {
        let replacement = format!("beta {i}");
        let modified = vec!["alpha", replacement.as_str(), "gamma"];
        let diff = compute(&original, &modified, &Options::default()).unwrap();
        assert_eq!(diff.changes.len(), 1);
    }
}
