//! What a row says, and what survives a narrow pane.
//!
//! The shape of the tree is checked in `explorer_tree.rs` and the placing in
//! `draw::buffer::explorer::node`; this is all of it together with a viewport
//! and a theme, which is the only place the whole thing can be wrong.
//!
//! The characters are asserted against a real screen here. The view
//! reports facts, and `draw::buffer::explorer` is what turns them into `│ `,
//! an icon and `M`.
//!
//! The icons are written as `\u{…}` escapes, as `theme::icon::table` writes
//! them, so a column in one of these literals is not a column on screen.

use crate::common::*;

#[test]
fn the_list_is_drawn_with_its_sections_guides_and_counts() {
    let mut session = TestSession::new(
        Buffer::explorer(entries()),
        Theme::named("basic-dark").unwrap(),
    );
    let rows = screen(&mut session, 44, 10);
    assert_eq!(
        rows,
        vec![
            "Changes (3 · +16 -3)".to_string(),
            "├ \u{e5fe} src".to_string(),
            // Directories before files, so the guides never cross a row that
            // is not under them.
            "│ ├ \u{e5fe} view".to_string(),
            "│ │ └ \u{e68b} tab.rs                          +4 M".to_string(),
            "│ └ \u{e68b} app.rs                        +12 -3 M".to_string(),
            "└ \u{f0219} notes.txt                             ??".to_string(),
            "Staged Changes (1 · +1 -1)".to_string(),
            "└ \u{f00ba} README.md                        +1 -1 M".to_string(),
            String::new(),
            // Row four, not row one: the reader starts on the first row they
            // can open, and rows one to three are a heading and two
            // directories.
            " changed files                          4/8".to_string(),
        ]
    );
}

#[test]
fn an_ancestor_that_was_last_leaves_blank_space_and_not_a_guide() {
    // The shared fixture cannot show this: it has no directory that is both
    // the last of its siblings and has children, so every guide column in it
    // is a `│`. Without a tree shaped like this one, a renderer that drew
    // `│ ` at every depth would pass every other test here.
    let mut session = TestSession::new(
        Buffer::explorer(vec![
            untracked("nest/a/one.txt"),
            untracked("nest/b/two.txt"),
        ]),
        Theme::named("basic-dark").unwrap(),
    );
    let rows = screen(&mut session, 30, 7);
    assert_eq!(
        &rows[..6],
        [
            "Changes (2)".to_string(),
            "└ \u{e5fe} nest".to_string(),
            // `nest` was the last of its siblings, so nothing runs down
            // beside it — two spaces, not `│ `.
            "  ├ \u{e5fe} a".to_string(),
            "  │ └ \u{f0219} one.txt             ??".to_string(),
            "  └ \u{e5fe} b".to_string(),
            "    └ \u{f0219} two.txt             ??".to_string(),
        ]
    );
}

#[test]
fn the_flat_shape_draws_whole_paths_and_no_guides() {
    // What VS Code's list mode does: no indent, no fold arrows, the whole
    // path on each line. A guide here would draw a tree where there is none.
    // See D69.
    let mut session = TestSession::new(
        Buffer::explorer(entries()),
        Theme::named("basic-dark").unwrap(),
    );
    session.press(crokey::key!(i));
    let rows = screen(&mut session, 44, 8);
    assert_eq!(
        &rows[..6],
        [
            "Changes (3 · +16 -3)".to_string(),
            "\u{f0219} notes.txt                               ??".to_string(),
            "\u{e68b} src/app.rs                        +12 -3 M".to_string(),
            "\u{e68b} src/view/tab.rs                       +4 M".to_string(),
            "Staged Changes (1 · +1 -1)".to_string(),
            "\u{f00ba} README.md                          +1 -1 M".to_string(),
        ]
    );
}

#[test]
fn the_reader_starts_on_the_first_file_and_not_on_a_heading() {
    // The failure this prevents: opening on the heading, where the key that
    // opens a file does nothing and the tool looks broken.
    let mut session = TestSession::new(
        Buffer::explorer(entries()),
        Theme::named("basic-dark").unwrap(),
    );
    let rows = screen(&mut session, 44, 10);
    assert!(rows[9].ends_with("4/8"), "{:?}", rows[9]);
}

#[test]
fn a_narrow_pane_keeps_the_name_and_the_status_and_drops_the_rest() {
    let mut session = TestSession::new(
        Buffer::explorer(entries()),
        Theme::named("basic-dark").unwrap(),
    );
    let rows = screen(&mut session, 20, 10);
    assert_eq!(
        &rows[..8],
        [
            "Changes (3 · +16 -3)",
            "├ \u{e5fe} src",
            "│ ├ \u{e5fe} view",
            "│ │ └ \u{e68b} tab.rs  +4 M",
            "│ └ \u{e68b} app.rs       M",
            "└ \u{f0219} notes.txt     ??",
            "Staged Changes",
            "└ \u{f00ba} README.md      M",
        ],
        "the name and the letter survive at twenty columns; the counts go \
         where there is no room for them"
    );
}

#[test]
fn a_pane_narrower_than_the_names_cuts_them_rather_than_wrapping() {
    let mut session = TestSession::new(
        Buffer::explorer(entries()),
        Theme::named("basic-dark").unwrap(),
    );
    let rows = screen(&mut session, 12, 10);
    // Every row still fits, and every row still says what happened to it.
    for row in &rows[..8] {
        assert!(row.chars().count() <= 12, "{row:?} overflows");
    }
    // Twelve columns is too few for the guides as well, and they are the last
    // thing dropped.
    assert_eq!(rows[4], "\u{e68b} app.rs   M");
}

#[test]
fn folding_a_directory_takes_its_files_off_the_list() {
    // `h` on a directory shuts it. The failure this prevents: a key bound in
    // the list that the interface never carries out, so the row stays open
    // and the tool looks broken.
    let mut session = TestSession::new(
        Buffer::explorer(entries()),
        Theme::named("basic-dark").unwrap(),
    );
    // Up from the first file to the `view` directory above it.
    session.press(crokey::key!(k));
    session.press(crokey::key!(h));

    let rows = screen(&mut session, 44, 10);
    assert!(
        !rows.iter().any(|row| row.contains("tab.rs")),
        "the folded directory still shows its file: {rows:?}"
    );
    assert!(
        rows.iter().any(|row| row.contains("app.rs")),
        "folding one directory shut the others too: {rows:?}"
    );
}

#[test]
fn a_second_fold_leaves_the_first_one_shut() {
    // The list is worked out afresh from the files and the reader's
    // arrangement. The failure this prevents: a fold arriving without the
    // ones before it, so shutting one directory opens the last.
    let mut session = TestSession::new(
        Buffer::explorer(vec![
            untracked("nest/a/one.txt"),
            untracked("nest/b/two.txt"),
        ]),
        Theme::named("basic-dark").unwrap(),
    );
    // Up from the first file to `a` and shut it, then down to `b` and shut
    // that.
    session.press(crokey::key!(k));
    session.press(crokey::key!(h));
    session.press(crokey::key!(j));
    session.press(crokey::key!(h));

    let rows = screen(&mut session, 30, 7);
    assert!(
        !rows.iter().any(|row| row.contains("one.txt")),
        "shutting the second directory opened the first: {rows:?}"
    );
    assert!(
        !rows.iter().any(|row| row.contains("two.txt")),
        "the second directory did not shut: {rows:?}"
    );
}

#[test]
fn a_file_added_above_the_reader_leaves_them_on_their_own_row() {
    // A row number means nothing across a rebuild, so the file is named
    // before and looked up after (D54). The failure this prevents: the
    // watcher seeing a new file and the cursor sliding onto another one.
    let mut session = TestSession::new(
        Buffer::explorer(vec![modified("b.rs"), modified("c.rs")]),
        Theme::named("basic-dark").unwrap(),
    );
    // Down from `b.rs` to `c.rs`, which is the row the refresh must keep.
    session.press(crokey::key!(j));

    session.refresh_list(vec![modified("a.rs"), modified("b.rs"), modified("c.rs")]);
    let rows = screen(&mut session, 44, 8);
    let landed = rows
        .iter()
        .position(|row| row.contains("c.rs"))
        .expect("c.rs is on screen") as u32;
    assert_eq!(
        session.cursor(),
        landed,
        "the reader was moved off c.rs: {rows:?}"
    );
}

#[test]
fn the_status_line_names_the_list_rather_than_a_file() {
    // The failure this prevents: showing the first file's name while the
    // reader is looking at all of them.
    let mut session = TestSession::new(
        Buffer::explorer(entries()),
        Theme::named("basic-dark").unwrap(),
    );
    let rows = screen(&mut session, 44, 4);
    assert!(
        rows[3].trim_start().starts_with("changed files"),
        "{:?}",
        rows[3]
    );
}

#[test]
fn an_empty_list_draws_nothing_rather_than_panicking() {
    // Reachable through a filter that matches no file. The binary refuses to
    // start on a clean tree, but nothing here may depend on that.
    let mut session = TestSession::new(
        Buffer::explorer(Vec::new()),
        Theme::named("basic-dark").unwrap(),
    );
    let rows = screen(&mut session, 40, 4);
    assert_eq!(rows[0], "");
}
