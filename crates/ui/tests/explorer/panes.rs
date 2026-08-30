//! Two panes: the box round each, which one a key means, and what opening a
//! file does to the one beside the list.

use crate::common::*;

#[test]
fn the_two_boxes_touch_with_no_gap_between_them() {
    let theme = Theme::named("catppuccin-mocha").unwrap();
    let mut session = scripted(
        only(vec![modified("src/lib.rs")]),
        theme,
        vec![single_file(unchanged("src/lib.rs"), "fn main() {}\n")],
    );
    open_selected(&mut session);

    let area = Rect::new(0, 0, 80, 10);
    let mut cells = Cells::empty(area);
    session.draw_into(&mut cells, area);

    // Left box right edge at 40, right box left edge at 41 — touching.
    for (x, top, bottom) in [(40, "╮", "╯"), (41, "╭", "╰")] {
        assert_eq!(cells[(x, 0)].symbol(), top);
        for y in 1..8 {
            assert_eq!(cells[(x, y)].symbol(), "│", "column {x} of row {y}");
        }
        assert_eq!(cells[(x, 8)].symbol(), bottom);
    }
}

#[test]
fn each_pane_is_drawn_in_a_box_and_the_focused_one_is_the_brighter() {
    let theme = Theme::named("catppuccin-mocha").unwrap();
    let mut session = scripted(
        only(vec![modified("src/lib.rs")]),
        theme,
        vec![single_file(unchanged("src/lib.rs"), "fn main() {}\n")],
    );
    open_selected(&mut session);

    let area = Rect::new(0, 0, 80, 10);
    let mut cells = Cells::empty(area);
    session.draw_into(&mut cells, area);

    // The four corners of each box.
    let symbol = |x: u16, y: u16| cells[(x, y)].symbol().to_owned();
    assert_eq!(
        [
            symbol(0, 0),
            symbol(40, 0),
            symbol(0, 8),
            symbol(40, 8),
            symbol(41, 0),
            symbol(79, 0),
            symbol(41, 8),
            symbol(79, 8)
        ],
        ["╭", "╮", "╰", "╯", "╭", "╮", "╰", "╯"]
    );

    // The list has focus, so its box is the focused colour and the diff's is
    // not.
    assert_eq!(cells[(0, 0)].fg, theme.border_focused.fg.unwrap());
    assert_eq!(cells[(40, 0)].fg, theme.border_focused.fg.unwrap());
    assert_eq!(cells[(41, 0)].fg, theme.border.fg.unwrap());
    assert_eq!(cells[(79, 0)].fg, theme.border.fg.unwrap());

    // And it moves with the focus.
    session.press(crokey::key!(right));
    session.draw_into(&mut cells, area);
    assert_eq!(cells[(0, 0)].fg, theme.border.fg.unwrap());
    assert_eq!(cells[(40, 0)].fg, theme.border.fg.unwrap());
    assert_eq!(cells[(41, 0)].fg, theme.border_focused.fg.unwrap());
    assert_eq!(cells[(79, 0)].fg, theme.border_focused.fg.unwrap());
}

#[test]
fn a_column_inside_each_box_is_left_clear_of_text() {
    // Text hard against the box is hard to read, so a column each side is
    // kept blank — and painted, so it is not a black stripe down a themed
    // background.
    let theme = Theme::named("catppuccin-mocha").unwrap();
    let mut session = scripted(
        only(vec![modified("src/lib.rs")]),
        theme,
        vec![single_file(unchanged("src/lib.rs"), "fn main() {}\n")],
    );
    open_selected(&mut session);

    let area = Rect::new(0, 0, 80, 10);
    let mut cells = Cells::empty(area);
    session.draw_into(&mut cells, area);

    // The list's box holds columns 0 and 40, the diff's 41 and 79, so the
    // clear columns are the ones beside those.
    let row: String = (0..80).map(|x| cells[(x, 1)].symbol()).collect();
    assert_eq!(column_of(&row, "Changes"), 2, "{row:?}");
    assert_eq!(column_of(&row, "fn main"), 43 + 4, "{row:?}");
    for x in [1, 39, 42, 78] {
        assert_eq!(cells[(x, 1)].symbol(), " ", "column {x} of {row:?}");
        assert_eq!(
            cells[(x, 1)].bg,
            theme.normal.bg.unwrap(),
            "column {x} was left unpainted"
        );
    }
}

#[test]
fn a_click_on_the_clear_column_beside_a_box_lands_nowhere() {
    use crossterm::event::{Event, MouseButton, MouseEvent, MouseEventKind};

    let theme = Theme::named("catppuccin-mocha").unwrap();
    let long: String = (0..100).map(|n| format!("line {n}\n")).collect();
    let mut session = scripted(
        only(vec![modified("src/lib.rs")]),
        theme,
        vec![diff(unchanged("src/lib.rs"), &long)],
    );
    open_selected(&mut session);

    let area = Rect::new(0, 0, 80, 10);
    let mut cells = Cells::empty(area);
    session.draw_into(&mut cells, area);
    let focus = session.view().tab().focus();

    for col in [1, 39, 42, 78] {
        session.handle_event(&Event::Mouse(MouseEvent {
            kind: MouseEventKind::Down(MouseButton::Left),
            column: col,
            row: 4,
            modifiers: crossterm::event::KeyModifiers::NONE,
        }));
        assert_eq!(
            session.view().tab().focus(),
            focus,
            "the click on column {col} moved the focus"
        );
    }
}

#[test]
fn a_body_with_no_room_for_a_box_is_drawn_without_one() {
    // Two columns of box and a row above and below is most of a screen this
    // size, and a pane with nothing in it says less than a squeezed one.
    let theme = Theme::named("catppuccin-mocha").unwrap();
    let mut session = scripted(
        only(vec![modified("a.rs")]),
        theme,
        vec![single_file(unchanged("a.rs"), "fn main() {}\n")],
    );
    open_selected(&mut session);
    let area = Rect::new(0, 0, 20, 3);
    let mut cells = Cells::empty(area);
    session.draw_into(&mut cells, area);
    let row: String = (0..20).map(|x| cells[(x, 0)].symbol()).collect();
    assert!(row.starts_with("Changes"), "{row:?}");

    // And every size below that draws something rather than panicking, with
    // two panes to place as well as one.
    for width in 1..8u16 {
        for height in 1..8u16 {
            let area = Rect::new(0, 0, width, height);
            let mut cells = Cells::empty(area);
            session.draw_into(&mut cells, area);
        }
    }
}

#[test]
fn the_layout_key_acts_on_the_diff_even_when_the_list_has_focus() {
    // The list has no layout to flip, so this key used to do nothing at all
    // while the list had focus — which is most of the time, and is a silent
    // key rather than an unbound one.
    let theme = Theme::named("catppuccin-mocha").unwrap();
    let mut session = scripted(
        only(vec![modified("src/lib.rs")]),
        theme,
        vec![diff(unchanged("src/lib.rs"), "fn main() {}\n")],
    );
    open_selected(&mut session);

    let area = Rect::new(0, 0, 80, 6);
    let before = drawn(&mut session, area);
    session.press(crokey::key!(t));
    let after = drawn(&mut session, area);
    assert_ne!(before, after, "the diff did not change layout");
}

#[test]
fn the_layout_key_does_nothing_when_there_is_no_diff_on_screen() {
    let theme = Theme::named("catppuccin-mocha").unwrap();
    let mut session = TestSession::new(Buffer::explorer(only(vec![modified("src/lib.rs")])), theme);
    let area = Rect::new(0, 0, 80, 6);
    let before = drawn(&mut session, area);
    session.press(crokey::key!(t));
    assert_eq!(drawn(&mut session, area), before);
}

/// The whole screen as text, for comparing one frame against another.
fn drawn(session: &mut TestSession, area: Rect) -> Vec<String> {
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
    // Ten thousand lines, because that is where the gutter reaches six columns
    // and two of them plus a divider plus the least readable text no longer
    // fit in what the split reserves.
    let long: String = (0..10_000).map(|n| format!("line {n}\n")).collect();
    let mut session = scripted(
        only(vec![modified("src/lib.rs")]),
        theme,
        vec![diff(unchanged("src/lib.rs"), &long)],
    );
    open_selected(&mut session);

    // The diff really is beside the list, so that the narrow widths below are
    // refusing a pane that exists. Without this the test passes with an empty
    // pane, which is the case it is not about.
    let row = inside(&mut session, 80, 6)[0].clone();
    assert!(row.contains("line 0"), "no diff to squeeze: {row:?}");

    for width in 24..40u16 {
        let area = Rect::new(0, 0, width, 6);
        let mut cells = Cells::empty(area);
        session.draw_into(&mut cells, area);
        let top: String = (0..width).map(|x| cells[(x, 0)].symbol()).collect();
        assert!(
            !top.starts_with("terminal too small"),
            "{width} columns gave up, though the list fits"
        );
        // The boxes are drawn before the panes are, so a fallback that did not
        // cover the body would leave the column the two shared standing in the
        // middle of the one pane that is left.
        let row = inside(&mut session, width, 6)[0].clone();
        assert!(
            row.starts_with("Changes") && !row.contains('│'),
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
    let long: String = (0..500).map(|n| format!("line {n}\n")).collect();
    // What the file becomes between the two openings: changed on disk, and
    // the same length.
    let edited: String = (0..500).map(|n| format!("edited {n}\n")).collect();
    let mut session = scripted(
        only(vec![modified("src/lib.rs")]),
        theme,
        vec![
            diff(unchanged("src/lib.rs"), &long),
            diff(unchanged("src/lib.rs"), &edited),
        ],
    );
    open_selected(&mut session);

    // Read to the bottom of the diff.
    let area = Rect::new(0, 0, 80, 8);
    let mut cells = Cells::empty(area);
    session.draw_into(&mut cells, area);
    session.press(crokey::key!(right));
    session.press(crokey::key!(shift - g));
    let far = session.view().focused().viewport.cursor();
    assert!(far > 400, "the cursor did not reach the end: {far}");

    // Back to the list, and enter on the row that is already open.
    session.press(crokey::key!(right));
    open_selected(&mut session);
    session.press(crokey::key!(right));
    assert_eq!(
        session.view().focused().viewport.cursor(),
        far,
        "the reader's place was thrown away"
    );

    // And the new bytes really are on screen.
    let row = inside(&mut session, 80, 8)[0].clone();
    assert!(
        row.contains("edited"),
        "the re-read did not happen: {row:?}"
    );
}

#[test]
fn re_opening_a_file_that_has_grown_shorter_lands_inside_it() {
    // The clamp: the reader was at line 499 and the file now has one line.
    let theme = Theme::named("catppuccin-mocha").unwrap();
    let long: String = (0..500).map(|n| format!("line {n}\n")).collect();
    let mut session = scripted(
        only(vec![modified("src/lib.rs")]),
        theme,
        vec![
            diff(unchanged("src/lib.rs"), &long),
            diff(unchanged("src/lib.rs"), "all that is left\n"),
        ],
    );
    open_selected(&mut session);
    session.press(crokey::key!(right));
    session.press(crokey::key!(shift - g));
    session.press(crokey::key!(right));

    open_selected(&mut session);
    let id = session
        .view()
        .tab()
        .right_pane_buffer()
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
    let in_unstaged = File::unchanged_path(at("both.rs"), Revs::new(Rev::Index, Rev::Worktree));
    let in_staged = File::unchanged_path(
        at("both.rs"),
        Revs::new(Rev::Commit(Oid::new("b87b24c")), Rev::Index),
    );
    let mut session = scripted(
        vec![in_unstaged.clone(), in_staged.clone()],
        theme,
        vec![
            single_file(in_unstaged.clone(), "the unstaged one\n"),
            single_file(in_staged.clone(), "the staged one\n"),
        ],
    );

    open_selected(&mut session);
    let first = shown_file(&session);
    // Down to the staged row, which is the same path in another section.
    session.press(crokey::key!(j));
    session.press(crokey::key!(j));
    open_selected(&mut session);
    let second = shown_file(&session);

    assert_eq!(first.path(), second.path(), "the same path");
    assert_ne!(
        first.rev(file_types::DiffVersion::Original),
        second.rev(file_types::DiffVersion::Original),
        "two comparisons, and the second really opened"
    );
}

#[test]
fn enter_on_a_file_row_asks_for_it() {
    // The key path, which nothing covered: every other test here calls
    // `open` directly, so a binding that reached the wrong level, or an arm
    // that folded a file instead of opening it, would go unnoticed.
    let theme = Theme::named("catppuccin-mocha").unwrap();
    let mut session = scripted(
        only(vec![modified("a.rs"), modified("b.rs")]),
        theme,
        vec![single_file(unchanged("b.rs"), "the second file\n")],
    );
    // Past the first row, so what is asked for is not what is already shown.
    session.press(crokey::key!(j));
    session.press(crokey::key!(enter));
    session.send_file_request();
    assert!(session.is_loading_file(), "enter asked for nothing");

    assert!(session.has_file_arrived(), "the answer was not installed");
    let row = inside(&mut session, 80, 6)[0].clone();
    assert!(row.contains("the second file"), "not on screen: {row:?}");
}

/// The file in the pane beside the list.
fn shown_file(session: &Session) -> File {
    let id = session
        .view()
        .tab()
        .right_pane_buffer()
        .expect("a pane beside the list");
    session
        .view()
        .buffer(id)
        .file()
        .expect("it shows a file")
        .clone()
}
