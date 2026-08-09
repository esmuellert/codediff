//! What a row says, and what survives a narrow pane.
//!
//! The shape of the tree is checked in `explorer_tree.rs` and the placing in
//! `draw::buffer::explorer::node`; this is all of it together with a viewport
//! and a theme, which is the only place the whole thing can be wrong.
//!
//! The characters are asserted against a real screen here. The view
//! reports facts, and `draw::buffer::explorer` is what turns them into `▾`,
//! `│ ` and `M`.

#[path = "explorer/common.rs"]
mod common;

use common::*;

#[test]
fn the_list_is_drawn_with_its_sections_guides_and_counts() {
    let mut session = Session::new(
        Buffer::explorer(entries()),
        Theme::named("basic-dark").unwrap(),
    );
    let rows = screen(&mut session, 44, 10);
    assert_eq!(
        rows,
        vec![
            "Changes (3 · +16 -3)".to_string(),
            "├ ▾ src".to_string(),
            // Directories before files, so the guides never cross a row that
            // is not under them.
            "│ ├ ▾ view".to_string(),
            "│ │ └ tab.rs                            +4 M".to_string(),
            "│ └ app.rs                          +12 -3 M".to_string(),
            "└ notes.txt                               ??".to_string(),
            "Staged Changes (1 · +1 -1)".to_string(),
            "└ README.md                          +1 -1 M".to_string(),
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
    let mut session = Session::new(
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
            "└ ▾ nest".to_string(),
            // `nest` was the last of its siblings, so nothing runs down
            // beside it — two spaces, not `│ `.
            "  ├ ▾ a".to_string(),
            "  │ └ one.txt               ??".to_string(),
            "  └ ▾ b".to_string(),
            "    └ two.txt               ??".to_string(),
        ]
    );
}

#[test]
fn the_flat_shape_draws_whole_paths_and_no_guides() {
    // What VS Code's list mode does: no indent, no fold arrows, the whole
    // path on each line. A guide here would draw a tree where there is none.
    // See D69.
    let mut session = Session::new(
        Buffer::explorer(entries()),
        Theme::named("basic-dark").unwrap(),
    );
    session.press(crokey::key!(i));
    let rows = screen(&mut session, 44, 8);
    assert_eq!(
        &rows[..6],
        [
            "Changes (3 · +16 -3)".to_string(),
            "notes.txt                                 ??".to_string(),
            "src/app.rs                          +12 -3 M".to_string(),
            "src/view/tab.rs                         +4 M".to_string(),
            "Staged Changes (1 · +1 -1)".to_string(),
            "README.md                            +1 -1 M".to_string(),
        ]
    );
}

#[test]
fn the_reader_starts_on_the_first_file_and_not_on_a_heading() {
    // The failure this prevents: opening on the heading, where the key that
    // opens a file does nothing and the tool looks broken.
    let mut session = Session::new(
        Buffer::explorer(entries()),
        Theme::named("basic-dark").unwrap(),
    );
    let rows = screen(&mut session, 44, 10);
    assert!(rows[9].ends_with("4/8"), "{:?}", rows[9]);
}

#[test]
fn a_narrow_pane_keeps_the_name_and_the_status_and_drops_the_rest() {
    let mut session = Session::new(
        Buffer::explorer(entries()),
        Theme::named("basic-dark").unwrap(),
    );
    let rows = screen(&mut session, 20, 10);
    assert_eq!(
        &rows[..8],
        [
            "Changes (3 · +16 -3)",
            "├ ▾ src",
            "│ ├ ▾ view",
            "│ │ └ tab.rs    +4 M",
            "│ └ app.rs  +12 -3 M",
            "└ notes.txt       ??",
            "Staged Changes",
            "└ README.md  +1 -1 M",
        ],
        "the counts fit at twenty columns, and the heading's do not"
    );
}

#[test]
fn a_pane_narrower_than_the_names_cuts_them_rather_than_wrapping() {
    let mut session = Session::new(
        Buffer::explorer(entries()),
        Theme::named("basic-dark").unwrap(),
    );
    let rows = screen(&mut session, 12, 10);
    // Every row still fits, and every row still says what happened to it.
    for row in &rows[..8] {
        assert!(row.chars().count() <= 12, "{row:?} overflows");
    }
    assert_eq!(rows[4], "│ └ app.rs M");
}

#[test]
fn the_status_line_names_the_list_rather_than_a_file() {
    // The failure this prevents: showing the first file's name while the
    // reader is looking at all of them.
    let mut session = Session::new(
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
    let mut session = Session::new(
        Buffer::explorer(Vec::new()),
        Theme::named("basic-dark").unwrap(),
    );
    let rows = screen(&mut session, 40, 4);
    assert_eq!(rows[0], "");
}
