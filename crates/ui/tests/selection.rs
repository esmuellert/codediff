//! Integration tests for mouse text selection.

#[path = "explorer/common.rs"]
mod common;

use common::*;
use crossterm::event::{Event, MouseButton, MouseEvent, MouseEventKind};
use ratatui::buffer::Buffer as Cells;
use ratatui::layout::Rect;
use ui::view::selection::{Selection, SelectionColumn};

/// Returns the active selection, or panics.
fn sel(session: &Session) -> &Selection {
    &session
        .view()
        .selection
        .as_ref()
        .expect("selection should exist")
        .1
}

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

fn setup_diff_session() -> Session {
    let theme = Theme::named("basic-dark").unwrap();
    let long: String = (0..50)
        .map(|n| format!("line {n} with some text\n"))
        .collect();
    let mut session = scripted(
        only(vec![modified("src/lib.rs")]),
        theme,
        vec![diff(unchanged("src/lib.rs"), &long)],
    );
    open_selected(&mut session);
    session.press(crokey::key!(right));
    draw(&mut session, 120, 30);
    session
}

#[test]
fn mouse_down_in_text_area_starts_selection() {
    let mut session = setup_diff_session();
    // Click alone does NOT create a selection — only records a pending.
    mouse(&mut session, MouseEventKind::Down(MouseButton::Left), 90, 3);
    assert!(
        session.view().selection.is_none(),
        "click alone should not create a selection"
    );

    // A drag promotes it to a real selection.
    mouse(&mut session, MouseEventKind::Drag(MouseButton::Left), 91, 3);
    assert!(
        session.view().selection.is_some(),
        "drag should create a selection"
    );
}

#[test]
fn mouse_drag_updates_selection_cursor() {
    let mut session = setup_diff_session();
    mouse(&mut session, MouseEventKind::Down(MouseButton::Left), 90, 3);
    mouse(
        &mut session,
        MouseEventKind::Drag(MouseButton::Left),
        100,
        5,
    );

    let sel = sel(&session);
    assert!(!sel.is_empty(), "selection should not be empty after drag");
    assert_ne!(sel.anchor, sel.cursor);
}

#[test]
fn mouse_up_finalizes_non_empty_selection() {
    let mut session = setup_diff_session();
    mouse(&mut session, MouseEventKind::Down(MouseButton::Left), 90, 3);
    mouse(
        &mut session,
        MouseEventKind::Drag(MouseButton::Left),
        100,
        5,
    );
    mouse(&mut session, MouseEventKind::Up(MouseButton::Left), 100, 5);

    assert!(
        session.view().selection.is_some(),
        "non-empty selection should persist after mouse-up"
    );
}

#[test]
fn mouse_up_clears_empty_selection() {
    let mut session = setup_diff_session();
    // Down and immediately up at the same position.
    mouse(&mut session, MouseEventKind::Down(MouseButton::Left), 90, 3);
    mouse(&mut session, MouseEventKind::Up(MouseButton::Left), 90, 3);

    assert!(
        session.view().selection.is_none(),
        "empty selection should be cleared on mouse-up"
    );
}

#[test]
fn clicking_elsewhere_clears_selection() {
    let mut session = setup_diff_session();
    // Create a selection.
    mouse(&mut session, MouseEventKind::Down(MouseButton::Left), 90, 3);
    mouse(
        &mut session,
        MouseEventKind::Drag(MouseButton::Left),
        100,
        5,
    );
    mouse(&mut session, MouseEventKind::Up(MouseButton::Left), 100, 5);
    assert!(session.view().selection.is_some());

    // Click somewhere else — starts a new (empty) selection, then up clears.
    mouse(
        &mut session,
        MouseEventKind::Down(MouseButton::Left),
        90,
        10,
    );
    mouse(&mut session, MouseEventKind::Up(MouseButton::Left), 90, 10);

    assert!(
        session.view().selection.is_none(),
        "old selection should be replaced and cleared by new click"
    );
}

#[test]
fn selection_is_confined_to_one_column() {
    let mut session = setup_diff_session();
    // Click and drag in the original column.
    mouse(&mut session, MouseEventKind::Down(MouseButton::Left), 48, 3);
    mouse(&mut session, MouseEventKind::Drag(MouseButton::Left), 55, 3);

    let s = sel(&session);
    assert_eq!(
        s.column,
        SelectionColumn::Original,
        "selection should be in the original column"
    );

    // Now click and drag in the modified column.
    mouse(&mut session, MouseEventKind::Down(MouseButton::Left), 95, 3);
    mouse(
        &mut session,
        MouseEventKind::Drag(MouseButton::Left),
        100,
        3,
    );

    let s = sel(&session);
    assert_eq!(
        s.column,
        SelectionColumn::Modified,
        "selection should be in the modified column"
    );
}

#[test]
fn selection_coordinates_are_buffer_local() {
    let mut session = setup_diff_session();

    // Scroll down first so viewport.top() > 0.
    for _ in 0..5 {
        mouse(&mut session, MouseEventKind::ScrollDown, 90, 5);
    }
    draw(&mut session, 120, 30);

    let top_before = session.view().focused().viewport.top();
    assert!(top_before > 0, "precondition: scrolled down");

    // Click and drag to create a selection.
    mouse(&mut session, MouseEventKind::Down(MouseButton::Left), 90, 2);
    mouse(&mut session, MouseEventKind::Drag(MouseButton::Left), 95, 2);

    let sel = sel(&session);
    assert!(
        sel.anchor.line >= top_before,
        "selection line {} should include scroll offset (top={})",
        sel.anchor.line,
        top_before
    );
}

#[test]
fn selection_highlight_appears_in_rendered_cells() {
    let theme = Theme::named("basic-dark").unwrap();
    let text: String = (0..20)
        .map(|n| format!("line {n} content here\n"))
        .collect();
    let mut session = scripted(
        only(vec![modified("src/lib.rs")]),
        theme,
        vec![diff(unchanged("src/lib.rs"), &text)],
    );
    open_selected(&mut session);
    session.press(crokey::key!(right));

    let area = Rect::new(0, 0, 120, 20);
    let mut cells = Cells::empty(area);
    session.draw_into(&mut cells, area);

    // Start a selection covering a few cells in the modified text area.
    mouse(&mut session, MouseEventKind::Down(MouseButton::Left), 90, 3);
    mouse(
        &mut session,
        MouseEventKind::Drag(MouseButton::Left),
        100,
        3,
    );

    // Re-draw to apply the selection highlight.
    let mut cells = Cells::empty(area);
    session.draw_into(&mut cells, area);

    // The selection style for basic-dark is bg=Indexed(17) (DARK_BLUE).
    let sel_bg = ratatui::style::Color::Indexed(17);
    let mut found_selection_bg = false;
    for x in 90..=100 {
        if let Some(cell) = cells.cell((x, 3)) {
            if cell.bg == sel_bg {
                found_selection_bg = true;
                break;
            }
        }
    }
    assert!(
        found_selection_bg,
        "expected selection highlight background in the selected cells"
    );
}

#[test]
fn clicking_explorer_opens_diff_pane() {
    let theme = Theme::named("basic-dark").unwrap();
    let text_a: String = (0..10).map(|n| format!("line a{n}\n")).collect();
    let text_b: String = (0..10).map(|n| format!("line b{n}\n")).collect();
    let mut session = scripted(
        only(vec![modified("src/app.rs"), modified("src/view.rs")]),
        theme,
        vec![
            diff(unchanged("src/app.rs"), &text_a),
            diff(unchanged("src/view.rs"), &text_b),
        ],
    );

    // Draw to populate hit_map. The tab is Full layout with explorer filling
    // the body.
    draw(&mut session, 80, 20);

    // The cursor starts on the first file (buffer.start_row()). That row is
    // guaranteed to be a file by the Explorer's own start_row logic.
    let file_row = session.view().focused().viewport.cursor() as u16;

    // Click that row. The explorer text area in Full layout starts after a
    // gutter. Col 10 is well within the text area.
    mouse(
        &mut session,
        MouseEventKind::Down(MouseButton::Left),
        10,
        file_row,
    );

    // Drive the file worker to completion.
    assert!(
        session.has_file_arrived(),
        "clicking a file in the explorer should trigger a file load"
    );

    // The tab should now be split — a diff pane appeared.
    assert!(
        session.view().tab().is_split(),
        "clicking a file in the explorer should open its diff pane"
    );
}

// --- Bug regression tests ---

#[test]
fn selection_cleared_on_layout_toggle() {
    let mut session = setup_diff_session();
    // Create a selection.
    mouse(&mut session, MouseEventKind::Down(MouseButton::Left), 90, 3);
    mouse(
        &mut session,
        MouseEventKind::Drag(MouseButton::Left),
        100,
        5,
    );
    mouse(&mut session, MouseEventKind::Up(MouseButton::Left), 100, 5);
    assert!(session.view().selection.is_some(), "precondition");

    // Toggle layout (side-by-side → inline).
    session.press(crokey::key!(t));

    assert!(
        session.view().selection.is_none(),
        "selection must be cleared when layout changes"
    );
}

#[test]
fn selection_cleared_on_buffer_change() {
    let theme = Theme::named("basic-dark").unwrap();
    let long: String = (0..50).map(|n| format!("line {n}\n")).collect();
    let short: String = (0..3).map(|n| format!("short {n}\n")).collect();
    let mut session = scripted(
        only(vec![modified("src/a.rs"), modified("src/b.rs")]),
        theme,
        vec![
            diff(unchanged("src/a.rs"), &long),
            diff(unchanged("src/b.rs"), &short),
        ],
    );
    open_selected(&mut session);
    session.press(crokey::key!(right));
    draw(&mut session, 120, 30);

    // Select something in the first file.
    mouse(
        &mut session,
        MouseEventKind::Down(MouseButton::Left),
        90,
        10,
    );
    mouse(
        &mut session,
        MouseEventKind::Drag(MouseButton::Left),
        100,
        12,
    );
    mouse(&mut session, MouseEventKind::Up(MouseButton::Left), 100, 12);
    assert!(session.view().selection.is_some(), "precondition");

    // Switch to explorer and open the second file.
    session.press(crokey::key!(left));
    session.press(crokey::key!(j));
    open_selected(&mut session);

    assert!(
        session.view().selection.is_none(),
        "selection must be cleared when a new file is opened"
    );
}

#[test]
fn selection_highlight_matches_click_position() {
    let mut session = setup_diff_session();
    // Scroll down so top > SCROLLOFF.
    for _ in 0..10 {
        mouse(&mut session, MouseEventKind::ScrollDown, 90, 5);
    }
    draw(&mut session, 120, 30);

    let top = session.view().focused().viewport.top();
    assert!(top > 3, "precondition: scrolled past scrolloff");

    // Click row 5, then drag to row 5 col+1 to create a 1-cell selection.
    mouse(&mut session, MouseEventKind::Down(MouseButton::Left), 90, 5);
    mouse(&mut session, MouseEventKind::Drag(MouseButton::Left), 91, 5);

    // The anchor line should correspond to screen row 5.
    let sel = sel(&session);
    // After place(), top may shift due to SCROLLOFF — but the anchor must
    // still match the view_line at screen row 5 when drawn.
    let area = Rect::new(0, 0, 120, 30);
    let mut cells = Cells::empty(area);
    session.draw_into(&mut cells, area);

    // The highlight should appear at row 5 (where we clicked).
    let sel_bg = ratatui::style::Color::Indexed(17);
    let highlighted = cells.cell((90, 5)).unwrap().bg == sel_bg;
    assert!(
        highlighted,
        "highlight must appear at the row where the user clicked"
    );
}

// --- Stronger highlight extent tests ---

#[test]
fn highlight_exact_extent() {
    let theme = Theme::named("basic-dark").unwrap();
    let text: String = (0..20).map(|n| format!("line {n} content\n")).collect();
    let mut session = scripted(
        only(vec![modified("src/lib.rs")]),
        theme,
        vec![diff(unchanged("src/lib.rs"), &text)],
    );
    open_selected(&mut session);
    session.press(crokey::key!(right));

    let area = Rect::new(0, 0, 120, 20);
    let mut cells = Cells::empty(area);
    session.draw_into(&mut cells, area);

    // Use screen_map to get the exact text rect.
    let ta = session
        .screen_map()
        .text_area_of(session.view().tab().focus(), SelectionColumn::Modified)
        .expect("modified text area should exist");
    let text_x = ta.rect.x;

    // Select from col 3 to col 8 on row 3 (relative to text area).
    let click_x_start = text_x + 3;
    let click_x_end = text_x + 8;
    mouse(
        &mut session,
        MouseEventKind::Down(MouseButton::Left),
        click_x_start,
        3,
    );
    mouse(
        &mut session,
        MouseEventKind::Drag(MouseButton::Left),
        click_x_end,
        3,
    );

    // Re-draw.
    let mut cells = Cells::empty(area);
    session.draw_into(&mut cells, area);

    let sel_bg = ratatui::style::Color::Indexed(17);

    // All cells in the range should be highlighted.
    for x in click_x_start..=click_x_end {
        let cell = cells.cell((x, 3)).unwrap();
        assert_eq!(cell.bg, sel_bg, "cell at x={x} row=3 should be highlighted");
    }
    // Cell just before and just after should NOT be highlighted.
    if click_x_start > 0 {
        let cell = cells.cell((click_x_start - 1, 3)).unwrap();
        assert_ne!(
            cell.bg,
            sel_bg,
            "cell at x={} row=3 should NOT be highlighted",
            click_x_start - 1
        );
    }
    let cell = cells.cell((click_x_end + 1, 3)).unwrap();
    assert_ne!(
        cell.bg,
        sel_bg,
        "cell at x={} row=3 should NOT be highlighted",
        click_x_end + 1
    );
    // Row above and below should NOT be highlighted.
    let cell = cells.cell((click_x_start + 2, 2)).unwrap();
    assert_ne!(cell.bg, sel_bg, "row 2 should NOT be highlighted");
    let cell = cells.cell((click_x_start + 2, 4)).unwrap();
    assert_ne!(cell.bg, sel_bg, "row 4 should NOT be highlighted");
}

#[test]
fn selection_does_not_highlight_other_pane() {
    let mut session = setup_diff_session();

    // Get the original column text area x range.
    let orig_ta = session
        .screen_map()
        .text_area_of(session.view().tab().focus(), SelectionColumn::Original)
        .expect("original text area");
    let orig_rect = orig_ta.rect;

    // Select in the modified column.
    mouse(&mut session, MouseEventKind::Down(MouseButton::Left), 90, 3);
    mouse(
        &mut session,
        MouseEventKind::Drag(MouseButton::Left),
        100,
        5,
    );

    let area = Rect::new(0, 0, 120, 30);
    let mut cells = Cells::empty(area);
    session.draw_into(&mut cells, area);

    let sel_bg = ratatui::style::Color::Indexed(17);
    // No cell in the original column should be highlighted.
    for y in orig_rect.y..orig_rect.bottom() {
        for x in orig_rect.x..orig_rect.right() {
            let cell = cells.cell((x, y)).unwrap();
            assert_ne!(
                cell.bg, sel_bg,
                "original column cell at ({x},{y}) must not be highlighted"
            );
        }
    }
}
