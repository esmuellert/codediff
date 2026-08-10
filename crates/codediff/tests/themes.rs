//! Which colours reach the screen.
//!
//! Separate from `screens.rs`, which asks what is *drawn*: a theme must change
//! every colour and no character, and the two questions have no assertions in
//! common.

mod harness;

use harness::{cells, diff, screen, session};
use ui::testing::TestSession;
use ui::{Session, Theme};

const BEFORE: &str = "one\ntwo\nthree\nfour\nfive";
const AFTER: &str = "one\nTWO\nthree\ninserted\nfour\nfive";

/// The same diff in a named theme.
fn themed(name: &str) -> TestSession {
    TestSession::new(
        diff("src/demo.rs", BEFORE, AFTER),
        Theme::named(name).expect(name),
    )
}

#[test]
fn a_changed_line_is_coloured_and_its_changed_characters_more_so() {
    let mut s = session("src/demo.rs", BEFORE, AFTER);
    let grid = cells(&mut s, 44, 8);
    let theme = Theme::DARK;

    // View line 1 is `two` against `TWO`. The whole line carries the delete colour on
    // the left and the insert colour on the right; the letters that actually
    // differ carry the stronger pair.
    let left_text = 4;
    assert_eq!(
        grid[(left_text, 1)].style().bg,
        theme.deleted_text.bg,
        "the changed characters"
    );
    assert_eq!(
        grid[(left_text + 3, 1)].style().bg,
        theme.deleted.bg,
        "the rest of the changed line"
    );

    let right_text = 26;
    assert_eq!(grid[(right_text, 1)].style().bg, theme.inserted_text.bg);
    assert_eq!(grid[(right_text + 3, 1)].style().bg, theme.inserted.bg);

    // An unchanged line keeps the ordinary background.
    assert_eq!(grid[(left_text, 2)].style().bg, theme.normal.bg);
}

#[test]
fn a_changed_line_is_coloured_to_the_edge_of_its_column() {
    // `hl_eol`: a highlight that stopped at the last character would make a
    // short changed line read as a ragged stripe rather than a marked line.
    let mut s = session("src/demo.rs", BEFORE, AFTER);
    let grid = cells(&mut s, 44, 8);
    assert_eq!(
        grid[(20, 1)].style().bg,
        Theme::DARK.deleted.bg,
        "the last column of the left column"
    );
}

#[test]
fn a_theme_changes_the_colours_and_nothing_else() {
    let mut plain = session("src/demo.rs", BEFORE, AFTER);
    let plain = screen(&mut plain, 44, 8);

    for name in Theme::NAMES {
        let mut s = themed(name);
        assert_eq!(screen(&mut s, 44, 8), plain, "{name} moved something");
    }
}

#[test]
fn every_theme_marks_the_changed_rows_differently_from_the_unchanged_ones() {
    for name in Theme::NAMES {
        let mut s = themed(name);
        let grid = cells(&mut s, 44, 8);

        // View line 1 changed, view line 2 did not; line 1's changed characters are
        // stronger still. All three must be distinguishable on screen, or the
        // theme is decorative rather than useful.
        let changed = grid[(7, 1)].style().bg;
        let inner = grid[(4, 1)].style().bg;
        let unchanged = grid[(4, 2)].style().bg;
        assert_ne!(changed, unchanged, "{name}: a changed line looks unchanged");
        assert_ne!(inner, changed, "{name}: no inner-change highlight");
    }
}

#[test]
fn the_basic_themes_never_send_a_24_bit_colour() {
    // They exist for terminals that cannot show one. A single `Rgb` would be
    // rendered as an escape the terminal ignores or, worse, prints.
    for name in ["basic-dark", "basic-light"] {
        let mut s = themed(name);
        let grid = cells(&mut s, 44, 8);
        for y in 0..8 {
            for x in 0..44 {
                let style = grid[(x, y)].style();
                for colour in [style.fg, style.bg] {
                    assert!(
                        !matches!(colour, Some(ui::ratatui::style::Color::Rgb(..))),
                        "{name} at {x},{y}: {colour:?}"
                    );
                }
            }
        }
    }
}
