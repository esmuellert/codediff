//! Keys reaching a real session.
//!
//! `ui`'s own tests prove the resolver in isolation — it is a pure
//! function, so they feed it keys and read commands. These prove the *wiring*:
//! that a keypress reaches the viewport, that a count survives the journey,
//! and that the three kinds of command are answered by three different things.

mod harness;

use harness::{measure, session, type_keys};
use ui::Flow;

/// Ten lines: one unchanged block, a change, more unchanged, another change.
const BEFORE: &str = "a1\na2\na3\na4\na5\na6\na7\na8\na9\na10";
const AFTER: &str = "a1\nCHANGED\na3\na4\na5\na6\na7\nALSO\na9\na10";

macro_rules! open {
    ($name:ident) => {
        #[allow(unused_mut)]
        let mut $name = {
            let mut s = session("demo.rs", BEFORE, AFTER);
            measure(&mut s);
            s
        };
    };
}

fn cursor(session: &ui::Session) -> u32 {
    session.view().focused().viewport.cursor()
}

fn cursor_after(keys: &str) -> u32 {
    open!(s);
    type_keys(&mut s, keys);
    cursor(&s)
}

fn left_after(keys: &str) -> u32 {
    open!(s);
    type_keys(&mut s, keys);
    s.view().focused().viewport.left()
}

#[test]
fn a_count_reaches_the_viewport() {
    assert_eq!(cursor_after("j"), 1);
    assert_eq!(cursor_after("jjj"), 3);
    assert_eq!(cursor_after("5j"), 5, "5j is five downs, not one");
    assert_eq!(cursor_after("12j"), 9, "clamped to the last line");
}

#[test]
fn zero_is_a_digit_mid_count_and_a_motion_otherwise() {
    // The single point where counts and bindings interact, so both halves have
    // to be observed. `50j` must be fifty downs rather than "scroll home" then
    // one down — and a bare `0` must still scroll, or it would be a digit that
    // silently starts a count of nothing.
    assert_eq!(cursor_after("5j"), 5);
    assert_eq!(cursor_after("50j"), 9, "fifty downs, clamped");

    assert!(left_after("lll") > 0, "scrolled right");
    assert_eq!(left_after("lll0"), 0, "a bare 0 goes back to column zero");
    assert_eq!(cursor_after("lll0"), 0, "and is not a motion");
}

#[test]
fn a_count_on_g_names_a_row_rather_than_repeating() {
    assert_eq!(cursor_after("G"), 9);
    assert_eq!(cursor_after("5G"), 4, "line five, 1-based on screen");
}

#[test]
fn escape_takes_back_a_pending_sequence_instead_of_quitting() {
    // The reason Esc is intercepted before the table: without it, pressing `g`
    // and changing your mind would exit the program.
    open!(s);
    assert_eq!(
        type_keys(&mut s, "g\u{1b}"),
        Flow::Continue,
        "must not quit"
    );
    assert_eq!(type_keys(&mut s, "jj"), Flow::Continue);
    assert_eq!(cursor(&s), 2, "the g did not survive");
}

#[test]
fn escape_takes_back_a_count_too() {
    // The flow matters as much as the position. Without the interception, the
    // escape reaches the table with a count of five attached — and the table
    // says quit.
    open!(s);
    assert_eq!(
        type_keys(&mut s, "5\u{1b}"),
        Flow::Continue,
        "must not quit"
    );
    assert_eq!(type_keys(&mut s, "jj"), Flow::Continue);
    assert_eq!(cursor(&s), 2, "two downs, not ten");
}

#[test]
fn escape_with_nothing_in_flight_still_quits() {
    open!(s);
    assert_eq!(type_keys(&mut s, "\u{1b}"), Flow::Quit);
}

#[test]
fn an_abandoned_sequence_does_not_leak_into_the_next_key() {
    assert_eq!(
        cursor_after("gxjj"),
        2,
        "gx is unbound; the two js still count"
    );
}

#[test]
fn gg_and_g_are_different_things() {
    open!(s);
    type_keys(&mut s, "G");
    assert_eq!(cursor(&s), 9);

    type_keys(&mut s, "g");
    assert_eq!(cursor(&s), 9, "one g is not a motion");
    type_keys(&mut s, "g");
    assert_eq!(cursor(&s), 0);
}

#[test]
fn a_count_steps_that_many_changes() {
    // `2]c` must land where `]c]c` lands.
    assert_eq!(cursor_after("]c]c"), cursor_after("2]c"));
    assert_ne!(cursor_after("]c"), cursor_after("2]c"));
}

#[test]
fn the_three_kinds_of_command_are_answered_by_three_different_things() {
    open!(s);
    // A buffer command moves the viewport and the loop carries on.
    assert_eq!(type_keys(&mut s, "j"), Flow::Continue);
    assert_eq!(cursor(&s), 1);
    // A program command does not touch the view, and the loop acts on it.
    assert_eq!(type_keys(&mut s, "q"), Flow::Quit);
    assert_eq!(cursor(&s), 1, "quitting is not a motion");
    // A task command cannot be produced yet — `TaskAction` is uninhabited, which is
    // checked by the compiler rather than here.
}

#[test]
fn an_unbound_key_does_nothing_at_all() {
    open!(s);
    for keys in ["z", "Z", "!", "\u{1}"] {
        assert_eq!(type_keys(&mut s, keys), Flow::Continue, "{keys:?} quit");
    }
    assert_eq!(cursor(&s), 0);
}

#[test]
fn toggling_the_layout_keeps_the_reader_on_the_same_line() {
    // The whole difficulty of the toggle: view line 40 side by side is a
    // different file line inline, so the number cannot be carried across. The
    // file line can.
    let mut s = session("f.rs", BEFORE, AFTER);
    measure(&mut s);
    type_keys(&mut s, "]c]c");
    let side_by_side = cursor(&s);
    let line = line_under_cursor(&s);

    type_keys(&mut s, "t");
    assert_ne!(cursor(&s), side_by_side, "the line must have moved");
    assert_eq!(line_under_cursor(&s), line, "but not to a different line");

    type_keys(&mut s, "t");
    assert_eq!(cursor(&s), side_by_side, "and back again");
    assert_eq!(line_under_cursor(&s), line);
}

#[test]
fn toggling_changes_how_many_view_lines_there_are() {
    // Not a cosmetic switch: a change costs the sum of its sides inline and
    // the taller of them side by side.
    let mut s = session("f.rs", BEFORE, AFTER);
    measure(&mut s);
    let columns = lines(&s);
    type_keys(&mut s, "t");
    assert!(lines(&s) > columns, "{} vs {columns}", lines(&s));
}
#[test]
fn the_divider_keys_do_nothing_inline() {
    // `>` widens the original column, which inline does not have, so it is
    // not bound there. Pressing it must be inert: no error, no stuck pending
    // sequence, and nothing on screen moves.
    let mut s = session("f.rs", BEFORE, AFTER);
    measure(&mut s);
    type_keys(&mut s, "t");
    let before = screen_line(&mut s);
    type_keys(&mut s, ">><<");
    assert_eq!(screen_line(&mut s), before, "a divider key drew something");
}

#[test]
fn the_divider_does_not_survive_a_trip_through_inline() {
    // It belongs to `SideBySide`, and inline has no columns to divide, so
    // there is nowhere for it to wait. Keeping it would mean a field `Inline`
    // carries and never reads, at which point the two are one type and neither
    // name means anything. Pressing `t` is a reader saying they do not want
    // columns; coming back to the default split answers that. See D35.
    let mut s = session("f.rs", BEFORE, AFTER);
    measure(&mut s);
    let default = screen_line(&mut s);
    type_keys(&mut s, ">>");
    assert_ne!(screen_line(&mut s), default, "`>` did nothing");
    type_keys(&mut s, "tt");
    assert_eq!(screen_line(&mut s), default);
}

#[test]
fn change_navigation_works_the_same_in_both_layouts() {
    // `]c` is bound once and shared, so the two cannot drift; the blocks it
    // steps through are computed per line layout, so it stops in the right
    // places in each.
    let mut s = session("f.rs", BEFORE, AFTER);
    measure(&mut s);
    type_keys(&mut s, "t]c");
    let first = line_under_cursor(&s);
    type_keys(&mut s, "]c");
    let second = line_under_cursor(&s);
    assert_ne!(first, second, "the second `]c` went nowhere");
    type_keys(&mut s, "]c");
    assert_eq!(line_under_cursor(&s), second, "there is no third change");
}

/// The file line the cursor is on, as text, whichever layout is in use.
fn line_under_cursor(session: &ui::Session) -> String {
    let buffer = session.view().focused_buffer();
    let alignment = buffer.alignment().expect("these tests build a diff");
    let layout = buffer.layout().expect("these tests build a diff");
    let (version, line) = alignment
        .line_at(layout, cursor(session))
        .expect("the cursor is on a line");
    alignment.line(version, line).expect("it exists").to_owned()
}

fn lines(session: &ui::Session) -> u32 {
    session.view().focused_buffer().view_lines()
}

fn screen_line(session: &mut ui::Session) -> String {
    harness::screen(session, 44, 4)
}
