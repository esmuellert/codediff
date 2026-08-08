//! What each part of a row is coloured, and when the colours are asked for.
//!
//! Both halves of the same fault: the list came out entirely in the ordinary
//! text colour, and the diff beside it stayed plain until a key was pressed.

#![allow(dead_code, unused_imports)]

#[path = "explorer/common.rs"]
mod common;

use common::*;

/// The colour of the first cell of each row, as the terminal would receive it.
fn colours(session: &mut Session, width: u16, height: u16, column: u16) -> Vec<Color> {
    let area = Rect::new(0, 0, width, height);
    let mut cells = Cells::empty(area);
    session.draw_into(&mut cells, area);
    (0..height).map(|y| cells[(column, y)].fg).collect()
}

#[test]
fn every_part_of_a_row_is_coloured_by_what_it_is() {
    // The failure this prevents: the whole list drawn in the ordinary text
    // colour, because the styles it borrowed were status-line patches with no
    // foreground and diff backgrounds with no foreground either.
    let theme = Theme::named("basic-dark").unwrap();
    let tree = theme.tree;
    let change = theme.change;
    let mut session = Session::new(Buffer::explorer(entries()), theme);

    // Row 0 `Changes (3 · +16 -3)`, row 1 `├ ▾ src`, row 5 `└ notes.txt … ??`.
    let first = colours(&mut session, 44, 8, 0);
    assert_eq!(first[0], tree.heading, "the section heading");
    assert_eq!(first[1], tree.marker, "an indent guide");

    // Column four of row 1 is the `s` of `src`; of row 5, the `o` of `notes`.
    let fourth = colours(&mut session, 44, 8, 4);
    assert_eq!(fourth[1], tree.directory, "a directory name");
    assert_eq!(fourth[5], tree.name, "a file name");

    // The last column is the status letter.
    let last = colours(&mut session, 44, 8, 43);
    assert_eq!(last[3], change.modified, "a modified file");
    assert_eq!(last[5], change.untracked, "an untracked file");

    // And the counts, at the end of row 3: `+4 M`.
    let counts = colours(&mut session, 44, 8, 40);
    assert_eq!(counts[3], change.gained, "lines gained");
    assert_ne!(change.gained, change.lost, "and green is not red");
}

#[test]
fn every_status_letter_has_a_colour_of_its_own() {
    let theme = Theme::named("basic-dark").unwrap();
    let change = theme.change;
    let files = vec![
        untracked("new.txt"),
        ChangedFile::reported(
            File::unchanged_path(at("clash.rs"), revs()),
            ChangeType::Conflicted,
        ),
        ChangedFile::new(File::added(at("added.rs"), staged_revs())),
        ChangedFile::new(File::unchanged_path(at("edited.rs"), staged_revs())),
        ChangedFile::new(File::deleted(at("gone.rs"), staged_revs())),
        ChangedFile::new(File::renamed(at("was.rs"), at("now.rs"), staged_revs())),
    ];
    let mut session = Session::new(Buffer::explorer(files), theme);
    let letters = colours(&mut session, 40, 12, 39);
    // Unstaged in name order, then staged in name order.
    assert_eq!(letters[1], change.conflicted, "clash.rs");
    assert_eq!(letters[2], change.untracked, "new.txt");
    assert_eq!(letters[4], change.added, "added.rs");
    assert_eq!(letters[5], change.modified, "edited.rs");
    assert_eq!(letters[6], change.deleted, "gone.rs");
    assert_eq!(letters[7], change.renamed, "now.rs");

    // All six are distinct, which is the whole point of the column.
    let all = [
        change.conflicted,
        change.untracked,
        change.added,
        change.modified,
        change.deleted,
        change.renamed,
    ];
    for (index, colour) in all.iter().enumerate() {
        assert!(
            !all[index + 1..].contains(colour),
            "two letters share {colour:?}"
        );
    }
}

#[test]
fn a_pane_that_does_not_have_focus_is_still_coloured() {
    // The failure this prevents: the diff drawn in plain text for as long as
    // the list has focus, which is most of the time. Two causes, both real —
    // colours were asked for only the focused pane, and opening a buffer asked
    // for nothing at all.
    // Catppuccin, not the basic theme: there a comment, a line number and an
    // indent guide are all DarkGray, so any search wide enough to find the
    // comment also finds the list beside it. This test passed twice with the
    // fix removed before that was noticed.
    let theme = Theme::named("catppuccin-mocha").unwrap();
    let comment = theme.code.comment;
    assert_ne!(
        comment,
        theme.line_number.fg.unwrap(),
        "or nothing is proved"
    );
    let mut session = scripted(
        only(vec![modified("src/lib.rs")]),
        theme,
        vec![single_file(
            unchanged("src/lib.rs"),
            "// a comment\nfn main() {}\n",
        )],
    );
    open_selected(&mut session);
    session.settle();

    // The list keeps focus, exactly as it does at startup.
    let area = Rect::new(0, 0, 80, 6);
    let mut cells = Cells::empty(area);
    session.draw_into(&mut cells, area);

    // The exact cells the comment is written in, found by its text.
    let row: String = (0..80).map(|x| cells[(x, 0)].symbol()).collect();
    let at = column_of(&row, "// a comment");
    assert_eq!(
        cells[(at, 0)].fg,
        comment,
        "the unfocused pane is not coloured"
    );
}

#[test]
fn re_opening_a_file_whose_bytes_changed_does_not_reuse_its_old_colours() {
    // A working-tree file has no id git can give it, so its name does not
    // change when its bytes do. The colour store answered a re-read with the
    // colours of what the file used to be — the same fault as the diff cache
    // D51 removed, one layer up.
    let theme = Theme::named("catppuccin-mocha").unwrap();
    let comment = theme.code.comment;
    let keyword = theme.code.keyword;
    assert_ne!(comment, keyword, "or nothing is proved");

    // Two files, because opening the one already shown is refused — the
    // reader's place in it is worth more than a re-read they did not ask for.
    // Going away and coming back is how the same name is asked for twice.
    let mut session = scripted(
        only(vec![modified("a.rs"), modified("b.rs")]),
        theme,
        vec![
            single_file(unchanged("a.rs"), "fn main() {}\n"),
            single_file(unchanged("b.rs"), "struct Other;\n"),
            // The first file again, by the same name, with different bytes.
            single_file(unchanged("a.rs"), "// now a comment\n"),
        ],
    );
    let area = Rect::new(0, 0, 80, 6);
    let mut cells = Cells::empty(area);

    open_selected(&mut session);
    session.settle();
    session.draw_into(&mut cells, area);
    let row: String = (0..80).map(|x| cells[(x, 0)].symbol()).collect();
    assert_eq!(
        cells[(column_of(&row, "fn"), 0)].fg,
        keyword,
        "a keyword to begin with"
    );

    session.press(crokey::key!(j));
    open_selected(&mut session);
    session.settle();
    session.press(crokey::key!(k));

    // The first file again, by the same name, with different bytes behind it.
    open_selected(&mut session);
    session.settle();
    session.draw_into(&mut cells, area);
    let row: String = (0..80).map(|x| cells[(x, 0)].symbol()).collect();
    assert_eq!(
        cells[(column_of(&row, "//"), 0)].fg,
        comment,
        "the new bytes wearing the old colours"
    );
}

#[test]
fn a_heading_and_a_status_letter_are_bold_in_every_theme() {
    // Weight is structural rather than a taste: a `Tree` holds `Color` and
    // cannot express it, so it is applied where the pieces are built. Nothing
    // asserted it there, and it survived a move only by luck.
    use ratatui::style::Modifier;

    for name in ["basic-dark", "catppuccin-mocha"] {
        let mut session = Session::new(Buffer::explorer(entries()), Theme::named(name).unwrap());
        let area = Rect::new(0, 0, 44, 8);
        let mut cells = Cells::empty(area);
        session.draw_into(&mut cells, area);

        let bold = |x: u16, y: u16| cells[(x, y)].modifier.contains(Modifier::BOLD);
        assert!(bold(0, 0), "{name}: the `C` of the section heading");
        assert!(bold(43, 3), "{name}: the status letter of a file");
        assert!(!bold(0, 1), "{name}: an indent guide is not bold");
        assert!(!bold(4, 5), "{name}: a file name is not bold");
    }
}
