//! End-to-end use of the public API.
//!
//! This file is an integration test, so it compiles against `vscode-diff` the
//! way any other crate would: only `pub` items are visible, and there is no
//! `unsafe`, no marshalling and no manual free. If these pass, Rust code can
//! use the C engine directly.

use vscode_diff::{DiffVersion, Error, Options, compute};

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
    assert_eq!(
        (change.original.start_line, change.original.end_line),
        (2, 4)
    );
    assert_eq!(
        (change.modified.start_line, change.modified.end_line),
        (2, 5)
    );
    assert_eq!(change.original.len(), 2);
    assert_eq!(change.modified.len(), 3);

    assert!(
        !change.inner_changes.is_empty(),
        "an edit within lines should carry character-level detail"
    );
}

#[test]
fn editor_lines_remove_every_form_of_line_ending() {
    assert_eq!(
        vscode_diff::editor_lines("alpha\r\nbeta\rgamma\n"),
        ["alpha", "beta", "gamma", ""]
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
    let inner = &diff.changes[0].inner_changes;
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
        (
            diff.changes[0].modified.start_line,
            diff.changes[0].modified.end_line
        ),
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
    assert_eq!(diff.changes[0].original.start_line, 2_501);
    assert!(diff.changes[1].is_insertion());
}

#[test]
fn binary_content_is_rejected_rather_than_truncated() {
    let err = compute(&["ok"], &["has\0nul"], &Options::default()).unwrap_err();
    assert_eq!(
        err,
        Error::InteriorNul {
            version: DiffVersion::Modified,
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

#[test]
fn a_single_line_break_does_not_join_long_character_diffs() {
    let original = vscode_diff::lines(concat!(
        "    let cwd = std::env::current_dir().context(\"finding the current directory\")?;\n",
        "    let mut git = Git::open(&cwd).context(\"opening a repository\")?;\n",
        "\n",
        "    let file = find(&mut git, path)?;\n",
        "    let before = git.before(&file).context(\"reading the before side\")?;\n",
        "    let after = git.after(&file).context(\"reading the after side\")?;\n",
        "\n",
        "    header(&file, &before, &after);",
    ));
    let modified = vscode_diff::lines(concat!(
        "    let runner = Runner::new(&find(path)?)?;\n",
        "    let contents = &runner.contents;\n",
        "    header(&contents.file, &contents.original, &contents.modified);",
    ));
    let options = Options::default()
        .ignoring_trim_whitespace()
        .with_time_budget_ms(0);
    let diff = compute(&original, &modified, &options).unwrap();

    assert_eq!(diff.changes.len(), 1);
    assert_eq!(diff.changes[0].inner_changes.len(), 5);
}

#[test]
fn utf16_tree_glyphs_do_not_truncate_character_heuristics() {
    let original = vscode_diff::lines(concat!(
        "│   ├── vscode-diff/          safe wrapper → Diff                      pure\n",
        "│   ├── metrics/              text measurement + coordinate mapping    pure\n",
        "│   ├── syntax/               text → normalized syntactic spans        pure\n",
        "│   ├── align/                AlignedDoc · rows · hunks · projections  pure\n",
        "│   ├── explorer/             entries · grouping · tree · filter       pure\n",
        "│   ├── vcs/                  git today, jj tomorrow\n",
        "│   ├── runtime/              events · commands · effects · watcher\n",
        "│   ├── display/              ratatui rendering + input\n",
        "│   ├── codediff/             binary · composition root\n",
        "│   └── fixtures/             dev-only: builds test repositories, emits a manifest\n",
        "├── xtask/                    lint, sync, verify and generate tasks (not a build system)\n",
        "├── libvscode-diff/           C source, copied from a pinned upstream tag\n",
        "└── docs/",
    ));
    let modified = vscode_diff::lines(concat!(
        "│   ├── diff-types/           the six structs a diff is made of — no deps, no C\n",
        "│   ├── file-types/           what a file under review is — no deps\n",
        "│   ├── vscode-diff/          safe wrapper → LinesDiff\n",
        "│   ├── line-index/           UTF-16 ↔ byte ↔ char ↔ cell, display width, tabs\n",
        "│   ├── syntax/               language detection, text → coloured spans\n",
        "│   ├── align/                pairing lines, fillers, hunks, inner-change spans\n",
        "│   ├── explorer/             file list: tree, flat mode, grouping, filtering\n",
        "│   ├── vcs/                  git subprocess: status, blob reads, cat-file\n",
        "│   ├── pipeline/             wires vcs + vscode-diff + align for one file\n",
        "│   ├── ui/                   terminal, input, rendering, theme, syntax worker\n",
        "│   ├── codediff/             binary — argument parsing, wiring\n",
        "│   └── fixtures/             dev-only: builds test git repositories\n",
        "├── xtask/                    lint-arch, lint-size, verify-c, fixture-repo, dev\n",
        "├── libvscode-diff/           C source, pinned to an upstream tag\n",
        "└── docs/",
    ));
    let options = Options::default()
        .ignoring_trim_whitespace()
        .with_time_budget_ms(0);
    let diff = compute(&original, &modified, &options).unwrap();

    assert_eq!(diff.changes.len(), 1);
    assert_eq!(diff.changes[0].inner_changes.len(), 14);
}

#[test]
fn long_myers_diagonals_match_vscode_typed_array_growth() {
    let original = vscode_diff::lines(concat!(
        "**Decision.** `j k h l Ctrl-D Ctrl-U gg G ]c [c Tab Enter / n N q ?` and counts. Nothing\n",
        "else.\n",
        "\n",
        "**Rationale.** Cursor, viewport and motions are entirely net-new work that Neovim previously\n",
        "supplied for free; it is 500–1,000 lines to do convincingly. Reimplementing Vim is an\n",
        "unbounded commitment that contributes nothing to the core thesis. Additional motions can be\n",
        "added on demand, from evidence.\n",
    ));
    let modified = vscode_diff::lines(concat!(
        "`j k h l Ctrl-D Ctrl-U gg G ]c [c Tab Enter / n N q ?` and counts. Nothing\n",
        "else. Additional motions added on demand from evidence.\n",
    ));
    let options = Options::default()
        .ignoring_trim_whitespace()
        .with_time_budget_ms(0);
    let diff = compute(&original, &modified, &options).unwrap();

    let inner = &diff.changes[0].inner_changes;
    assert_eq!(inner.len(), 4);
    assert_eq!(inner[1].original.start_line, 2);
    assert_eq!(inner[1].original.start_col, 6);
    assert_eq!(inner[1].original.end_line, 6);
    assert_eq!(inner[1].original.end_col, 77);
    assert_eq!(inner[1].modified.start_line, 2);
    assert_eq!(inner[1].modified.start_col, 6);
    assert_eq!(inner[1].modified.end_col, 17);
}
