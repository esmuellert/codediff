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

    let cursor_before = session.view().focused().viewport.cursor();
    let top_before = session.view().focused().viewport.top();
    assert!(cursor_before > 0, "precondition: cursor not at top");

    // Scroll down with the mouse hovering over the focused (diff) pane.
    mouse(&mut session, MouseEventKind::ScrollDown, 60, 3);

    let cursor_after = session.view().focused().viewport.cursor();
    let top_after = session.view().focused().viewport.top();

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

    // Focus is on the explorer (left pane, PaneId(0), columns 0–39).
    let focused_id = session.view().tab().focus();

    // The diff is the other pane.
    let diff_pane_id = session
        .view()
        .tab()
        .ids()
        .find(|&id| id != focused_id)
        .expect("a second pane");

    // Scroll with the mouse hovering over the diff pane (right side, ~col 60),
    // which is NOT focused.
    mouse(&mut session, MouseEventKind::ScrollDown, 60, 3);

    // Focus should NOT have changed.
    assert_eq!(
        session.view().tab().focus(),
        focused_id,
        "scroll must not change focus"
    );

    // The diff pane's top should have moved.
    let diff_top = session.view().tab().pane(diff_pane_id).viewport.top();
    assert!(
        diff_top > 0,
        "the hovered (unfocused) pane should have scrolled, top = {diff_top}"
    );

    // The focused pane (explorer) should NOT have scrolled.
    let explorer_top = session.view().focused().viewport.top();
    assert_eq!(explorer_top, 0, "the focused pane should not have scrolled");
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

    assert_eq!(
        session
            .view()
            .selected_file()
            .map(|file| file.path().as_str().to_owned()),
        Some("src/zeta.rs".to_owned()),
        "the click did not land on the new file"
    );
}

#[test]
fn a_click_on_the_box_round_a_pane_lands_nowhere() {
    // The box belongs to no pane, so the reader cannot focus one by clicking
    // its edge — which is what recording the inside of each box, rather than
    // the whole of it, is for.
    let theme = Theme::named("basic-dark").unwrap();
    let long: String = (0..100).map(|n| format!("line {n}\n")).collect();
    let mut session = scripted(
        only(vec![modified("src/lib.rs")]),
        theme,
        vec![diff(unchanged("src/lib.rs"), &long)],
    );
    open_selected(&mut session);
    draw(&mut session, 80, 12);
    let focus = session.view().tab().focus();

    // The top edge of the diff's box, its right edge, and the column the two
    // boxes share.
    for (col, row) in [(60, 0), (79, 5), (40, 5)] {
        mouse(
            &mut session,
            MouseEventKind::Down(MouseButton::Left),
            col,
            row,
        );
        assert_eq!(
            session.view().tab().focus(),
            focus,
            "the click at ({col}, {row}) moved the focus"
        );
    }
}

#[test]
fn a_click_below_a_shortened_list_lands_nowhere() {
    let theme = Theme::named("basic-dark").unwrap();
    let mut session = scripted(
        only(vec![modified("src/lib.rs"), modified("src/zeta.rs")]),
        theme,
        vec![],
    );
    let rows = inside(&mut session, 80, 12);
    let row = rows
        .iter()
        .position(|row| row.contains("zeta.rs"))
        .expect("both files are on screen") as u16;

    session.refresh_list(vec![modified("src/lib.rs")]);
    // How many rows the shorter list draws, read off the screen rather than
    // asked of the buffer, which is the thing under test.
    let rows = inside(&mut session, 80, 12);
    let drawn = rows.iter().position(String::is_empty).expect("blank rows") as u32;

    // That row is now past the end of the list. One further down than the
    // list says, because the box's top edge is above it.
    mouse(
        &mut session,
        MouseEventKind::Down(MouseButton::Left),
        4,
        row + 1,
    );

    assert!(
        session.view().focused().viewport.cursor() < drawn,
        "the cursor left the list: {} of {drawn} rows",
        session.view().focused().viewport.cursor()
    );
}
