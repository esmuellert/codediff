//! What each kind of buffer looks like on screen.
//!
//! A file with two sides is a diff; a file with one is not a diff with an
//! empty column, it is a different kind of buffer. These tests hold that line:
//! a one-sided file has no divider, no fillers and no highlighting, because
//! nothing here changed *relative to* anything. VSCode reached the same place
//! and stopped opening a diff editor for added, untracked and deleted files —
//! an empty left-hand side "did not provide much value". See D23 and D27.

mod harness;

use harness::{added, cells, key, measure, screen, single};
use ui::crossterm::event::KeyCode;
use ui::{Session, Theme};

#[test]
fn a_file_with_nothing_to_compare_against_gets_one_column() {
    // `(added)` is derived from the file existing on one side only. Nothing
    // passes that string in, which is why the status line can style it
    // separately from the path.
    let mut s = Session::new(added("new.rs", "alpha\nbeta"), Theme::DARK);
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
    let mut s = Session::new(single("new.rs", "alpha\nbeta"), Theme::DARK);
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
    // Motions are arithmetic over a line count, so every buffer kind gets them
    // without implementing one. If this ever needed its own code, the line
    // between generic motions and buffer-specific ones would be in the wrong
    // place.
    let long = (1..=50)
        .map(|i| format!("line {i}"))
        .collect::<Vec<_>>()
        .join("\n");
    let mut s = Session::new(single("big.rs", &long), Theme::DARK);
    measure(&mut s);
    s.handle_event(&key(KeyCode::Char('G')));
    assert_eq!(s.view().focused().viewport.cursor(), 49);
    assert!(screen(&mut s, 40, 6).contains("line 50"));
}

#[test]
fn the_keys_a_one_sided_file_cannot_use_do_nothing() {
    // `]c` and `>` are not bound in this keymap_type — there are no changes to
    // step through and no second column to resize. Pressing them must be inert
    // rather than an error or a stuck pending sequence.
    let mut s = Session::new(single("new.rs", "alpha\nbeta"), Theme::DARK);
    let before = screen(&mut s, 40, 4);
    for c in [']', 'c', '[', 'c', '>', '<'] {
        s.handle_event(&key(KeyCode::Char(c)));
    }
    assert_eq!(screen(&mut s, 40, 4), before);
}

#[test]
fn the_added_note_is_not_styled_as_though_it_were_the_path() {
    // The bug that motivated `File`. When the status line was handed
    // `"new.rs   (added)"` as one string in a field called `path`, it rendered
    // the whole thing in the path's bold style — including a note that is not
    // part of any path, and which no caller could then shorten or restyle.
    let mut s = Session::new(added("new.rs", "alpha"), Theme::DARK);
    let grid = cells(&mut s, 40, 3);
    let row = 2;
    let status: String = (0..40).map(|x| grid[(x, row)].symbol()).collect();

    let name_at = status.find("new.rs").expect("the name is drawn") as u16;
    let note_at = status.find("(added)").expect("the note is drawn") as u16;

    assert_eq!(
        grid[(name_at, row)].style().add_modifier,
        Theme::DARK.status_path.add_modifier,
        "the path keeps its own style"
    );
    assert_ne!(
        grid[(note_at, row)].style().add_modifier,
        grid[(name_at, row)].style().add_modifier,
        "the note must not inherit the path's style"
    );
}
