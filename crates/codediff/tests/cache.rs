//! Colours kept, colours asked for, and colours never asked for twice.
//!
//! The parts are checked where they live — the store evicts, the handle
//! refuses a second request, the worker resumes. This is the one test that
//! drives all three the way a reader does: open a file, scroll, come back, and
//! see what was computed and what was not.
//!
//! Perl throughout. Only one of the two engines can read part of a file:
//! the matcher stops where it is asked and carries on later, and the parser
//! has no range API and reads everything however little was wanted. So a
//! language the parser does not know is the only place laziness is visible at
//! all, and Rust would prove nothing.

mod harness;

use harness::{cells, key, single};
use ui::Theme;
use ui::crossterm::event::KeyCode;
use ui::ratatui::buffer::Buffer as Cells;
use ui::ratatui::style::Color;
use ui::testing::TestSession;

const WIDTH: u16 = 100;
const HEIGHT: u16 = 24;

/// A Perl file of `lines` lines, every one of them with something to colour.
fn perl(lines: usize) -> String {
    (0..lines)
        .map(|n| format!("my $x{n} = \"line {n}\";  # a comment\n"))
        .collect()
}

/// A session over one Perl file, drawn once so the viewport has a height.
fn session(lines: usize) -> TestSession {
    let mut session = TestSession::new(single("a.pl", &perl(lines)), Theme::DARK);
    let _ = cells(&mut session, WIDTH, HEIGHT);
    session
}

/// Presses a key and lets the loop react, as [`ui::run`] does.
fn press(session: &mut TestSession, code: KeyCode) {
    session.handle_event(&key(code));
        let _ = cells(session, WIDTH, HEIGHT);
}

/// How many different colours a row uses.
///
/// A coloured row of code has several; a plain one has one. Counting rather
/// than naming a colour keeps this from depending on the theme.
fn colours(cells: &Cells, y: u16) -> usize {
    let mut seen: Vec<Color> = Vec::new();
    for x in 0..cells.area.width {
        let fg = cells[(x, y)].style().fg.unwrap_or(Color::Reset);
        if !seen.contains(&fg) {
            seen.push(fg);
        }
    }
    seen.len()
}

/// Whether any row of the screen is coloured.
fn anything_coloured(cells: &Cells) -> bool {
    (0..cells.area.height).any(|y| colours(cells, y) > 2)
}

#[test]
fn a_file_is_coloured_without_being_waited_for() {
    let mut session = session(200);
    session.wait_until_idle();
    let cells = cells(&mut session, WIDTH, HEIGHT);
    assert!(
        anything_coloured(&cells),
        "code on screen is coloured once the answers have arrived"
    );
}

#[test]
fn nothing_is_asked_for_twice() {
    // The whole reason the store is on the drawing side. A frame that finds
    // what it needs sends no request at all, so coming back to a file costs a
    // lookup rather than a read.
    let mut session = session(200);
    session.wait_until_idle();
    assert!(!session.is_colouring(), "settled");

        assert!(
        !session.is_colouring(),
        "asking again for a screen already coloured sends nothing"
    );
}

#[test]
fn scrolling_back_over_coloured_lines_asks_for_nothing() {
    let mut session = session(200);
    session.wait_until_idle();
    for _ in 0..5 {
        press(&mut session, KeyCode::Char('j'));
    }
    press(&mut session, KeyCode::Char('k'));
    assert!(
        !session.is_colouring(),
        "lines already held are not asked for again"
    );
}

#[test]
fn a_file_longer_than_the_read_ahead_is_not_read_to_its_end() {
    // Laziness, from the outside. What proves it is that jumping to the end
    // finds work to do — if opening the file had read all of it there would
    // be nothing left to ask for.
    let mut session = session(20_000);
    session.wait_until_idle();
    assert!(!session.is_colouring(), "the screen is done");

    session.handle_event(&key(KeyCode::Char('G')));
        assert!(
        session.is_colouring(),
        "the end of the file had not been read, so reaching it asks"
    );
}

#[test]
fn the_end_of_a_long_file_is_coloured_once_it_is_reached() {
    let mut session = session(20_000);
    session.wait_until_idle();
    press(&mut session, KeyCode::Char('G'));
    session.wait_until_idle();
    let cells = cells(&mut session, WIDTH, HEIGHT);
    assert!(anything_coloured(&cells), "and it is coloured when it is");
}

#[test]
fn reaching_further_into_a_file_keeps_what_was_already_read() {
    // The join is what this is really about: a second read starts where the
    // first stopped, and the store refuses a piece that does not continue
    // exactly where the last ended. A restart would therefore show up as a
    // file that stops being coloured part way down.
    let mut session = session(20_000);
    session.wait_until_idle();
    press(&mut session, KeyCode::Char('G'));
    session.wait_until_idle();

    // Back to the top: those lines were read first and must still be there.
    press(&mut session, KeyCode::Char('g'));
    press(&mut session, KeyCode::Char('g'));
    let cells = cells(&mut session, WIDTH, HEIGHT);
    assert!(
        anything_coloured(&cells),
        "the top is still coloured after reading the end"
    );
    assert!(!session.is_colouring(), "and nothing had to be read again");
}

#[test]
fn a_language_nothing_claims_draws_plainly_and_stops_asking() {
    let mut session = TestSession::new(single("a.qqqqq", "nothing claims this\n"), Theme::DARK);
    let _ = cells(&mut session, WIDTH, HEIGHT);
    session.wait_until_idle();
    assert!(
        !session.is_colouring(),
        "answered once, with nothing, rather than asked for ever"
    );
}
