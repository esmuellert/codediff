//! What each kind of buffer looks like on screen.
//!
//! A file with two sides is a diff; a file with one is not a diff with an
//! empty column, it is a different kind of buffer. These tests hold that line:
//! a one-sided file has no divider, no fillers and no highlighting, because
//! nothing here changed *relative to* anything. VSCode reached the same place
//! and stopped opening a diff editor for added, untracked and deleted files —
//! an empty left-hand side "did not provide much value". See D23 and D27.

mod harness;

use harness::{cells, key, measure, screen, text};
use ui::crossterm::event::KeyCode;
use ui::{Session, Theme};

#[test]
fn a_file_with_nothing_to_compare_against_gets_one_column() {
    let mut s = Session::new(text("new.rs   (added)", "alpha\nbeta"), Theme::DARK);
    assert_eq!(
        screen(&mut s, 40, 4),
        [
            "  1 alpha                               ",
            "  2 beta                                ",
            "                                        ",
            " new.rs   (added)      no changes   1/2 ",
        ]
        .join("\n")
    );
}

#[test]
fn a_one_sided_file_is_drawn_in_the_ordinary_colours() {
    // The whole file is new, but nothing on it is *a change* — there is no
    // other side for it to differ from.
    let mut s = Session::new(text("new.rs", "alpha\nbeta"), Theme::DARK);
    let grid = cells(&mut s, 40, 4);
    for x in 0..40 {
        assert_eq!(
            grid[(x, 1)].style().bg,
            Theme::DARK.normal.bg,
            "column {x} of a one-sided file is coloured"
        );
    }
}

#[test]
fn a_one_sided_file_still_scrolls() {
    // Motions are arithmetic over a row count, so every buffer kind gets them
    // without implementing one. If this ever needed its own code, the line
    // between generic motions and buffer-specific ones would be in the wrong
    // place.
    let long = (1..=50)
        .map(|i| format!("line {i}"))
        .collect::<Vec<_>>()
        .join("\n");
    let mut s = Session::new(text("big.rs", &long), Theme::DARK);
    measure(&mut s);
    s.handle(&key(KeyCode::Char('G')));
    assert_eq!(s.view().focused().viewport.cursor(), 49);
    assert!(screen(&mut s, 40, 6).contains("line 50"));
}

#[test]
fn the_keys_a_one_sided_file_cannot_use_do_nothing() {
    // `n` and `>` are not bound in this context — there are no changes to step
    // through and no second column to resize. Pressing them must be inert
    // rather than an error or a stuck pending sequence.
    let mut s = Session::new(text("new.rs", "alpha\nbeta"), Theme::DARK);
    let before = screen(&mut s, 40, 4);
    for c in ['n', 'N', '>', '<'] {
        s.handle(&key(KeyCode::Char(c)));
    }
    assert_eq!(screen(&mut s, 40, 4), before);
}
