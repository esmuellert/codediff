//! Two panes: the border between them, which one a key means, and what
//! opening a file does to the one beside the list.

#![allow(dead_code, unused_imports)]

#[path = "explorer/common.rs"]
mod common;

use common::*;

#[test]
fn the_border_runs_the_whole_height_of_the_tab() {
    // The failure this prevents: `cells::hatch` draws one row, so a
    // full-height rectangle handed to it drew the border on the top line only
    // and left a blank column down the rest of the screen.
    let theme = Theme::named("catppuccin-mocha").unwrap();
    let mut session = Session::new(Buffer::explorer(only(vec![modified("src/lib.rs")])), theme);
    session.open(&mut Fake("fn main() {}\n"));

    let area = Rect::new(0, 0, 80, 10);
    let mut cells = Cells::empty(area);
    session.draw_into(&mut cells, area);

    // Every row of the body, which is everything above the status line.
    for y in 0..9 {
        assert_eq!(cells[(40, y)].symbol(), "│", "row {y} has no border");
    }
}

#[test]
fn the_layout_key_acts_on_the_diff_even_when_the_list_has_focus() {
    // The list has no layout to flip, so this key used to do nothing at all
    // while the list had focus — which is most of the time, and is a silent
    // key rather than an unbound one.
    let theme = Theme::named("catppuccin-mocha").unwrap();
    let mut session = Session::new(Buffer::explorer(only(vec![modified("src/lib.rs")])), theme);
    session.open(&mut FakeDiff("fn main() {}\n"));

    let area = Rect::new(0, 0, 80, 6);
    let before = drawn(&mut session, area);
    session.press(crokey::key!(t));
    let after = drawn(&mut session, area);
    assert_ne!(before, after, "the diff did not change layout");
}

#[test]
fn the_layout_key_does_nothing_when_there_is_no_diff_on_screen() {
    let theme = Theme::named("catppuccin-mocha").unwrap();
    let mut session = Session::new(Buffer::explorer(only(vec![modified("src/lib.rs")])), theme);
    let area = Rect::new(0, 0, 80, 6);
    let before = drawn(&mut session, area);
    session.press(crokey::key!(t));
    assert_eq!(drawn(&mut session, area), before);
}

/// The whole screen as text, for comparing one frame against another.
fn drawn(session: &mut Session, area: Rect) -> Vec<String> {
    let mut cells = Cells::empty(area);
    session.draw_into(&mut cells, area);
    (0..area.height)
        .map(|y| (0..area.width).map(|x| cells[(x, y)].symbol()).collect())
        .collect()
}

#[test]
fn a_pane_that_will_not_fit_falls_back_to_one_rather_than_failing_the_screen() {
    // Whether a diff fits depends on how wide its line numbers are, which the
    // rectangle arithmetic cannot know. A file with a seven-digit gutter made
    // the right pane refuse at 29 columns, and the refusal was turned into
    // "terminal too small" for the whole screen — while 28 columns drew the
    // list perfectly.
    let theme = Theme::named("catppuccin-mocha").unwrap();
    let mut session = Session::new(Buffer::explorer(only(vec![modified("src/lib.rs")])), theme);
    // Ten thousand lines, because that is where the gutter reaches six columns
    // and two of them plus a divider plus the least readable text no longer
    // fit in the twenty the split reserves.
    let long: String = (0..10_000).map(|n| format!("line {n}\n")).collect();
    session.open(&mut FakeDiff(Box::leak(long.into_boxed_str())));

    for width in 24..40u16 {
        let area = Rect::new(0, 0, width, 6);
        let mut cells = Cells::empty(area);
        session.draw_into(&mut cells, area);
        let row: String = (0..width).map(|x| cells[(x, 0)].symbol()).collect();
        assert!(
            !row.starts_with("terminal too small"),
            "{width} columns gave up, though the list fits"
        );
        // The border is hatched before the panes are drawn, so a fallback that
        // did not clear the body would leave a column of it standing in the
        // middle of a single pane.
        assert!(
            row.starts_with("Changes"),
            "{width} columns left something behind: {row:?}"
        );
    }
}

#[test]
fn opening_the_file_already_shown_re_reads_it_and_keeps_the_readers_place() {
    // Two requirements that pull against each other. A new pane starts at the
    // top, which is right for a different file and threw away the reader's
    // place for the same one. Refusing to re-open kept the place and made the
    // only gesture that re-reads a file silent, so a file edited elsewhere
    // could never be refreshed — the staleness D51 removed, one layer up. So:
    // re-read, then put the reader back.
    let theme = Theme::named("catppuccin-mocha").unwrap();
    let mut session = Session::new(Buffer::explorer(only(vec![modified("src/lib.rs")])), theme);
    let long: String = (0..500).map(|n| format!("line {n}\n")).collect();
    session.open(&mut FakeDiff(Box::leak(long.into_boxed_str())));

    // Read to the bottom of the diff.
    let area = Rect::new(0, 0, 80, 8);
    let mut cells = Cells::empty(area);
    session.draw_into(&mut cells, area);
    session.press(crokey::key!(tab));
    session.press(crokey::key!(shift - g));
    let far = session.view().focused().viewport.cursor();
    assert!(far > 400, "the cursor did not reach the end: {far}");

    // Back to the list, and enter on the row that is already open. The file
    // has changed on disk since, and is the same length.
    session.press(crokey::key!(tab));
    let edited: String = (0..500).map(|n| format!("edited {n}\n")).collect();
    session.open(&mut FakeDiff(Box::leak(edited.into_boxed_str())));
    session.press(crokey::key!(tab));
    assert_eq!(
        session.view().focused().viewport.cursor(),
        far,
        "the reader's place was thrown away"
    );

    // And the new bytes really are on screen.
    session.draw_into(&mut cells, area);
    let row: String = (0..80).map(|x| cells[(x, 0)].symbol()).collect();
    assert!(
        row.contains("edited"),
        "the re-read did not happen: {row:?}"
    );
}

#[test]
fn re_opening_a_file_that_has_grown_shorter_lands_inside_it() {
    // The clamp: the reader was at line 499 and the file now has one line.
    let theme = Theme::named("catppuccin-mocha").unwrap();
    let mut session = Session::new(Buffer::explorer(only(vec![modified("src/lib.rs")])), theme);
    let long: String = (0..500).map(|n| format!("line {n}\n")).collect();
    session.open(&mut FakeDiff(Box::leak(long.into_boxed_str())));
    session.press(crokey::key!(tab));
    session.press(crokey::key!(shift - g));
    session.press(crokey::key!(tab));

    session.open(&mut FakeDiff("all that is left\n"));
    let id = session
        .view()
        .tab()
        .shown()
        .expect("a pane beside the list");
    let rows = session.view().buffer(id).view_lines();
    assert!(
        session.view().pane_for(id).viewport.cursor() < rows,
        "the cursor is past the end of the file"
    );
}

#[test]
fn the_file_listed_twice_can_be_opened_from_either_section() {
    // A file staged and then edited again is two comparisons of one path. The
    // guard that refuses to re-open the file already shown compares revisions
    // as well as the path, so it must not mistake one row for the other.
    let theme = Theme::named("catppuccin-mocha").unwrap();
    // Each row is built with the revisions of *its own* group, which is what
    // makes them two comparisons rather than one file listed twice.
    let in_unstaged = ChangedFile::new(
        File::unchanged_path(at("both.rs"), Revs::new(Rev::Index, Rev::Worktree)),
        None,
    );
    let in_staged = ChangedFile::new(
        File::unchanged_path(
            at("both.rs"),
            Revs::new(Rev::Commit(Oid::new("b87b24c")), Rev::Index),
        ),
        None,
    );
    let mut session = Session::new(
        Buffer::explorer(vec![
            unstaged(vec![Entry::new(in_unstaged)]),
            staged(vec![Entry::new(in_staged)]),
        ]),
        theme,
    );

    session.open(&mut Fake("the unstaged one\n"));
    let first = shown_file(&session);
    // Down to the staged row, which is the same path in another section.
    session.press(crokey::key!(j));
    session.press(crokey::key!(j));
    session.open(&mut Fake("the staged one\n"));
    let second = shown_file(&session);

    assert_eq!(first.path(), second.path(), "the same path");
    assert_ne!(
        first.rev(file_types::DiffVersion::Original),
        second.rev(file_types::DiffVersion::Original),
        "two comparisons, and the second really opened"
    );
}

/// The file in the pane beside the list.
fn shown_file(session: &Session) -> File {
    let id = session
        .view()
        .tab()
        .shown()
        .expect("a pane beside the list");
    session
        .view()
        .buffer(id)
        .file()
        .expect("it shows a file")
        .clone()
}
