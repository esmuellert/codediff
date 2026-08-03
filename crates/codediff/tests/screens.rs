//! What the interface actually looks like.
//!
//! Rendered into an in-memory grid rather than onto a terminal, so the whole
//! screen can be asserted as text. The screens here are deliberately tiny —
//! 44 columns by 8 rows — because a snapshot nobody can read in a diff is a
//! snapshot nobody checks.
//!
//! Two notes on reading them. A double-width character occupies two grid
//! columns but appears once, followed by the space its second column holds;
//! and index arithmetic on these strings must count characters, not bytes,
//! since `╱` and `│` are three bytes each.
//!
//! Keys reaching a session are in `keys.rs`; this file is about pixels.

mod harness;

use harness::{cells, column_of, key, screen, session};
use ui::crossterm::event::KeyCode;
use ui::{Flow, Session, Theme};

const BEFORE: &str = "one\ntwo\nthree\nfour\nfive";
const AFTER: &str = "one\nTWO\nthree\ninserted\nfour\nfive";

fn demo() -> Session {
    session("src/demo.rs", BEFORE, AFTER)
}

#[test]
fn a_small_diff_side_by_side() {
    let mut s = demo();
    assert_eq!(
        screen(&mut s, 44, 8),
        [
            "  1 one              │  1 one               ",
            "  2 two              │  2 TWO               ",
            "  3 three            │  3 three             ",
            "╱╱╱╱╱╱╱╱╱╱╱╱╱╱╱╱╱╱╱╱╱│  4 inserted          ",
            "  4 four             │  5 four              ",
            "  5 five             │  6 five              ",
            "                     │                      ",
            " src/demo.rs                2 changes   1/6 ",
        ]
        .join("\n")
    );
}

#[test]
fn the_two_sides_never_show_different_rows() {
    // Not an aesthetic check. The left numbers skip 3→4 across the filler
    // while the right run 3,4,5 — which is only correct because both columns
    // were drawn from one row list.
    let mut s = demo();
    let rendered = screen(&mut s, 44, 8);
    let rows: Vec<&str> = rendered.lines().collect();
    assert!(rows[3].starts_with('╱'), "left filler: {:?}", rows[3]);
    assert!(rows[3].contains("4 inserted"), "{:?}", rows[3]);
    assert!(rows[4].contains("  4 four") && rows[4].contains("  5 four"));
}

#[test]
fn scrolling_moves_both_sides_together() {
    let mut s = demo();
    // Six rows of document, five rows of text: reaching the end must scroll.
    s.handle(&key(KeyCode::Char('G')));
    let rendered = screen(&mut s, 44, 6);
    let rows: Vec<&str> = rendered.lines().collect();
    assert!(rows[0].starts_with("  2 two"), "{:?}", rows[0]);
    assert!(
        rows[0].contains("  2 TWO"),
        "one scroll position, both sides"
    );
    assert!(rows[4].contains("  5 five") && rows[4].contains("  6 five"));
}

#[test]
fn columns_stay_aligned_across_a_double_width_character() {
    let mut s = session("wide.rs", "a日b\nx", "a日c\nx");
    let rendered = screen(&mut s, 40, 4);
    let rows: Vec<&str> = rendered.lines().collect();
    // If the wide character had been counted as one column, everything after
    // it on that row would have shifted and the divider would not line up.
    assert_eq!(
        column_of(rows[0], '│'),
        column_of(rows[1], '│'),
        "{rendered}"
    );
}

#[test]
fn an_escape_sequence_in_the_file_cannot_reach_the_terminal() {
    let mut s = session("evil.rs", "plain", "\u{1b}[31mgotcha");
    let rendered = screen(&mut s, 44, 4);
    assert!(!rendered.contains('\u{1b}'), "{rendered:?}");
    assert!(rendered.contains('\u{241b}'), "{rendered:?}");
}

#[test]
fn horizontal_scrolling_shifts_the_text_and_not_the_numbers() {
    let mut s = session("long.rs", "abcdefghijklmnop", "abcdefghijklmnoq");
    for _ in 0..2 {
        s.handle(&key(KeyCode::Char('l')));
    }
    let rendered = screen(&mut s, 44, 3);
    let rows: Vec<&str> = rendered.lines().collect();
    assert!(rows[0].starts_with("  1 ijklmnop"), "{:?}", rows[0]);
}

#[test]
fn an_inner_change_keeps_its_highlight_on_its_character_when_scrolled() {
    // The whole row is marked because the line changed; `9` is marked more
    // strongly because it is what changed. Scrolled sideways the stronger mark
    // has to travel with the character: the engine reports it as a byte offset
    // into the line, while the scroll is counted in screen cells, and on a
    // line with a tab or a wide character those two disagree.
    let mut s = session("f.rs", "fn f() { total = 1; }", "fn f() { total = 9; }");
    s.handle(&key(KeyCode::Char('l')));
    let grid = cells(&mut s, 44, 3);
    let row: String = (0..44).map(|x| grid[(x, 0)].symbol()).collect();

    // Without this the test would still pass if scrolling stopped working.
    assert!(!row.contains("fn f()"), "the view did not scroll: {row:?}");

    let at = column_of(&row, '9').expect("the changed character is on screen");
    let bg = |x: usize| grid[(x as u16, 0)].style().bg;
    assert_ne!(bg(at), bg(at - 1), "the mark reaches left of it: {row:?}");
    assert_ne!(bg(at), bg(at + 1), "the mark reaches right of it: {row:?}");
}

#[test]
fn dragging_the_divider_moves_it() {
    let mut s = demo();
    let first = screen(&mut s, 44, 8);
    let before = column_of(first.lines().next().unwrap(), '│');
    s.handle(&key(KeyCode::Char('>')));
    let second = screen(&mut s, 44, 8);
    let after = column_of(second.lines().next().unwrap(), '│');
    assert!(after > before, "{before:?} -> {after:?}");
}

#[test]
fn a_terminal_too_small_says_so_instead_of_drawing_rubbish() {
    let mut s = demo();
    let rendered = screen(&mut s, 12, 4);
    assert!(rendered.starts_with("terminal too"), "{rendered:?}");
}

#[test]
fn an_identical_file_renders_with_no_highlighting_and_says_so() {
    let mut s = session("same.rs", "same\nlines", "same\nlines");
    let rendered = screen(&mut s, 44, 4);
    assert!(rendered.contains("no changes"), "{rendered:?}");
    assert!(rendered.contains("  1 same"), "{rendered:?}");
}

#[test]
fn resizing_smaller_keeps_the_cursor_on_screen() {
    let mut s = demo();
    s.handle(&key(KeyCode::Char('G')));
    screen(&mut s, 44, 20);
    let rendered = screen(&mut s, 44, 4);
    assert!(rendered.contains("five"), "{rendered:?}");
    assert_eq!(s.view().focused().viewport.cursor(), 5);
}

#[test]
fn quitting_is_the_only_way_the_loop_ends() {
    let mut s = demo();
    for code in [
        KeyCode::Char('j'),
        KeyCode::Char('k'),
        KeyCode::Char('x'),
        KeyCode::Tab,
        KeyCode::F(5),
    ] {
        assert_eq!(
            s.handle(&key(code)),
            Flow::Continue,
            "{code:?} should not quit"
        );
    }
    assert_eq!(s.handle(&key(KeyCode::Char('q'))), Flow::Quit);
}

#[test]
fn navigation_and_the_status_line_count_the_same_changes() {
    // They used to disagree: the status line reported context-merged hunks
    // while `n` stepped through changed blocks, so a file could say "1" and
    // still stop twice.
    let mut s = demo();
    assert!(screen(&mut s, 44, 8).contains("2 changes"));

    s.handle(&key(KeyCode::Char('n')));
    assert_eq!(s.view().focused().viewport.cursor(), 1, "the two/TWO row");
    assert!(screen(&mut s, 44, 8).contains("change 1/2"));

    s.handle(&key(KeyCode::Char('n')));
    assert_eq!(s.view().focused().viewport.cursor(), 3, "the inserted row");
    assert!(screen(&mut s, 44, 8).contains("change 2/2"));
}

#[test]
fn a_diff_the_engine_abandoned_says_so_on_screen() {
    // The engine gives up after five seconds by default and returns a coarser
    // result. A reviewer who mistakes that for a complete diff approves code
    // they have not seen, so it has to reach the screen — and the only test
    // that covered it set the flag on the status line directly, which would
    // not have noticed the wire from the buffer being cut.
    let before = vscode_diff::lines(BEFORE);
    let after = vscode_diff::lines(AFTER);
    let real = vscode_diff::compute(&before, &after, &vscode_diff::Options::default())
        .expect("the engine runs");
    let abandoned = vscode_diff::LinesDiff {
        hit_timeout: true,
        ..real.clone()
    };

    let mut s = demo();
    assert!(
        !screen(&mut s, 60, 8).contains("PARTIAL"),
        "a complete diff must not claim to be partial"
    );

    let buffer = harness::with_diff("src/demo.rs", BEFORE, AFTER, abandoned);
    let mut s = Session::new(buffer, Theme::DARK);
    assert!(
        screen(&mut s, 60, 8).contains("PARTIAL"),
        "an abandoned diff was not announced"
    );
}
