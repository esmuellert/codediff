//! Mouse behaviour: scroll moves the view (not the cursor), and targets the
//! pane the mouse is hovering over (not the focused one).

use crate::common::*;
use crossterm::event::{Event, MouseButton, MouseEvent, MouseEventKind};
use ratatui::buffer::Buffer as Cells;
use ratatui::layout::Rect;
use ui::Session;

/// Sends a mouse event to the session.
fn mouse(session: &mut Session, kind: MouseEventKind, col: u16, row: u16) {
    let event = Event::Mouse(MouseEvent {
        kind,
        column: col,
        row,
        modifiers: crossterm::event::KeyModifiers::NONE,
    });
    session.handle_event(&event);
}

/// Draws a frame so the hit map is populated.
fn draw(session: &mut Session, width: u16, height: u16) {
    let area = Rect::new(0, 0, width, height);
    let mut cells = Cells::empty(area);
    session.draw_into(&mut cells, area);
}

/// The first view line the diff pane is showing, read off the screen.
///
/// The files these tests open name their own lines — `line 0`, `line 1`, … —
/// so the top row says which one it is. Read rather than asked, because
/// where a pane is scrolled to is not something the interface offers.
fn diff_top(session: &mut TestSession, width: u16, height: u16) -> u32 {
    let rows = screen(session, width, height);
    let row = rows.first().expect("a first row").clone();
    let at = row
        .find("line ")
        .unwrap_or_else(|| panic!("no diff on the top row: {row:?}"));
    row[at + "line ".len()..]
        .split_whitespace()
        .next()
        .and_then(|word| word.parse().ok())
        .unwrap_or_else(|| panic!("no line number in {row:?}"))
}

#[test]
fn scroll_moves_the_view_without_moving_the_cursor() {
    let theme = Theme::named("basic-dark").unwrap();
    // A file with many lines so there is something to scroll through.
    let long: String = (0..100).map(|n| format!("line {n}\n")).collect();
    let mut session = scripted(
        only(vec![modified("src/lib.rs")]),
        theme,
        vec![diff(unchanged("src/lib.rs"), &long)],
    );
    open_selected(&mut session);
    // Move focus to the diff pane.
    session.press(crokey::key!(right));
    draw(&mut session, 80, 10);

    // Move the cursor to the middle of the visible range so it won't be
    // clamped by the scroll.
    session.press(crokey::key!(j));
    session.press(crokey::key!(j));
    session.press(crokey::key!(j));
    session.press(crokey::key!(j));

    let cursor_before = session.cursor();
    let top_before = diff_top(&mut session, 80, 10);
    assert!(cursor_before > 0, "precondition: cursor not at top");

    // Scroll down with the mouse hovering over the focused (diff) pane.
    mouse(&mut session, MouseEventKind::ScrollDown, 60, 3);

    let cursor_after = session.cursor();
    let top_after = diff_top(&mut session, 80, 10);

    // The view moved down.
    assert!(
        top_after > top_before,
        "top should move: was {top_before}, now {top_after}"
    );
    // The cursor did NOT move — browser-style scroll. The cursor was in the
    // middle of the visible range, so it stays put.
    assert_eq!(
        cursor_before, cursor_after,
        "cursor should stay: was {cursor_before}, now {cursor_after}"
    );
}

#[test]
fn scroll_targets_the_hovered_pane_not_the_focused_one() {
    let theme = Theme::named("basic-dark").unwrap();
    let long: String = (0..100).map(|n| format!("line {n}\n")).collect();
    let mut session = scripted(
        only(vec![modified("src/lib.rs")]),
        theme,
        vec![diff(unchanged("src/lib.rs"), &long)],
    );
    open_selected(&mut session);
    draw(&mut session, 80, 10);

    // Focus stays on the list, exactly as it does at startup, so the diff is
    // the pane the pointer is over and not the pane the keys mean.
    let cursor_before = session.cursor();
    assert_eq!(diff_top(&mut session, 80, 10), 0, "precondition: at the top");

    // Scroll with the mouse hovering over the diff pane (right side, ~col 60),
    // which is NOT focused.
    mouse(&mut session, MouseEventKind::ScrollDown, 60, 3);

    assert!(
        diff_top(&mut session, 80, 10) > 0,
        "the hovered pane should have scrolled"
    );
    // The list still has the keys, and has not moved under them.
    assert_eq!(
        session.cursor(),
        cursor_before,
        "scroll must not change focus, nor move the focused pane"
    );
}

#[test]
fn a_file_a_refresh_added_can_be_clicked() {
    let theme = Theme::named("basic-dark").unwrap();
    let mut session = scripted(only(vec![modified("src/lib.rs")]), theme, vec![]);
    draw(&mut session, 80, 12);

    session.refresh_list(vec![modified("src/lib.rs"), modified("src/zeta.rs")]);
    let rows = screen(&mut session, 80, 12);
    let row = rows
        .iter()
        .position(|row| row.contains("zeta.rs"))
        .expect("the new file is on screen") as u16;

    mouse(
        &mut session,
        MouseEventKind::Down(MouseButton::Left),
        4,
        row,
    );

    // The list is at its top and starts at the first screen row, so the row
    // the click landed on is the view line the cursor should be on — and
    // where the cursor is, is the file the click asked to open.
    assert_eq!(
        session.cursor(),
        u32::from(row),
        "the click did not land on the new file"
    );
}

#[test]
fn a_click_in_the_list_takes_the_keys_back_from_the_diff() {
    // A press picks a row, and the row it picks is the one the keys then
    // move from. The failure this prevents: clicking a file while reading
    // one, and having `j` go on scrolling the diff.
    let theme = Theme::named("basic-dark").unwrap();
    let long: String = (0..100).map(|n| format!("line {n}\n")).collect();
    let mut session = scripted(
        only(vec![modified("src/lib.rs"), modified("src/zeta.rs")]),
        theme,
        vec![diff(unchanged("src/lib.rs"), &long)],
    );
    open_selected(&mut session);
    // Into the diff, and down it, so that the two panes are nowhere near
    // each other.
    session.press(crokey::key!(right));
    for _ in 0..8 {
        session.press(crokey::key!(j));
    }
    assert!(session.cursor() > 2, "precondition: the diff has the keys");

    let rows = screen(&mut session, 80, 12);
    let row = rows
        .iter()
        .position(|row| row.contains("zeta.rs"))
        .expect("the second file is on screen") as u16;
    mouse(
        &mut session,
        MouseEventKind::Down(MouseButton::Left),
        4,
        row,
    );

    assert_eq!(
        session.cursor(),
        u32::from(row),
        "the click did not move the cursor into the list"
    );
    session.press(crokey::key!(k));
    assert_eq!(
        session.cursor(),
        u32::from(row) - 1,
        "the key still went to the diff"
    );
}

#[test]
fn a_click_below_a_shortened_list_lands_nowhere() {
    let theme = Theme::named("basic-dark").unwrap();
    let mut session = scripted(
        only(vec![modified("src/lib.rs"), modified("src/zeta.rs")]),
        theme,
        vec![],
    );
    let rows = screen(&mut session, 80, 12);
    let row = rows
        .iter()
        .position(|row| row.contains("zeta.rs"))
        .expect("both files are on screen") as u16;

    session.refresh_list(vec![modified("src/lib.rs")]);
    // How many rows the shorter list draws, read off the screen rather than
    // asked of the buffer, which is the thing under test.
    let rows = screen(&mut session, 80, 12);
    let drawn = rows.iter().position(String::is_empty).expect("blank rows") as u32;

    // That row is now past the end of the list.
    mouse(
        &mut session,
        MouseEventKind::Down(MouseButton::Left),
        4,
        row,
    );

    assert!(
        session.cursor() < drawn,
        "the cursor left the list: {} of {drawn} rows",
        session.cursor()
    );
}
