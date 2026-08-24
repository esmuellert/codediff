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

use harness::{cells, column_of, key, screen, session, single, type_keys};
use ui::crossterm::event::KeyCode;
use ui::testing::TestSession;
use ui::{Flow, Theme};

const BEFORE: &str = "one\ntwo\nthree\nfour\nfive";
const AFTER: &str = "one\nTWO\nthree\ninserted\nfour\nfive";

fn demo() -> TestSession {
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
    s.handle_event(&key(KeyCode::Char('G')));
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
        s.handle_event(&key(KeyCode::Char('l')));
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
    s.handle_event(&key(KeyCode::Char('l')));
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
    s.handle_event(&key(KeyCode::Char('G')));
    screen(&mut s, 44, 20);
    let rendered = screen(&mut s, 44, 4);
    assert!(rendered.contains("five"), "{rendered:?}");
    assert_eq!(s.cursor(), 5);
}

#[test]
fn only_quitting_and_the_debug_rebuild_end_the_loop() {
    let mut s = demo();
    for code in [
        KeyCode::Char('j'),
        KeyCode::Char('k'),
        KeyCode::Char('x'),
        KeyCode::Tab,
        KeyCode::F(6),
    ] {
        assert_eq!(
            s.handle_event(&key(code)),
            Flow::Continue,
            "{code:?} should not quit"
        );
    }
    // A debug build has one more way out, and only there: F5 leaves so that
    // `cargo xtask dev` can rebuild.
    #[cfg(debug_assertions)]
    assert_eq!(s.handle_event(&key(KeyCode::F(5))), Flow::Rebuild);
    #[cfg(not(debug_assertions))]
    assert_eq!(s.handle_event(&key(KeyCode::F(5))), Flow::Continue);
    assert_eq!(s.handle_event(&key(KeyCode::Char('q'))), Flow::Quit);
}

#[test]
fn navigation_and_the_status_line_count_the_same_changes() {
    // They used to disagree: the status line reported keymap_type-merged hunks
    // while `]c` stepped through changed blocks, so a file could say "1" and
    // still stop twice.
    let mut s = demo();
    assert!(screen(&mut s, 44, 8).contains("2 changes"));

    type_keys(&mut s, "]c");
    assert_eq!(s.cursor(), 1, "the two/TWO row");
    assert!(screen(&mut s, 44, 8).contains("change 1/2"));

    type_keys(&mut s, "]c");
    assert_eq!(s.cursor(), 3, "the inserted row");
    assert!(screen(&mut s, 44, 8).contains("change 2/2"));
}

#[test]
fn a_change_key_at_the_last_change_says_so_rather_than_doing_nothing() {
    // Silence here reads as a broken key. Cycling round instead would be
    // worse: it destroys the one signal that matters when checking an agent's
    // work — that you have now seen everything. See S9.
    let mut s = demo();
    type_keys(&mut s, "]c]c");
    assert!(screen(&mut s, 44, 8).contains("change 2/2"));

    type_keys(&mut s, "]c");
    let stuck = screen(&mut s, 44, 8);
    assert!(stuck.contains("no next change"), "{stuck:?}");
    assert_eq!(
        s.cursor(),
        3,
        "it must not have moved"
    );

    // Any other key answers the note, which is how vim's echo area behaves and
    // why none of this needs a clock.
    type_keys(&mut s, "k");
    assert!(!screen(&mut s, 44, 8).contains("no next change"));
}

#[test]
fn the_same_at_the_first_change_going_backwards() {
    let mut s = demo();
    type_keys(&mut s, "[c");
    let stuck = screen(&mut s, 44, 8);
    assert!(stuck.contains("no previous change"), "{stuck:?}");
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
    let mut s = TestSession::new(buffer, Theme::DARK);
    assert!(
        screen(&mut s, 60, 8).contains("PARTIAL"),
        "an abandoned diff was not announced"
    );
}

#[test]
fn the_same_diff_read_inline() {
    // The same six lines as `a_small_diff_side_by_side`, one version per row:
    // what was there, then what replaced it. Two gutters, and the empty one
    // tells you which version a row shows — no sign column, because it would
    // only repeat them.
    let mut s = demo();
    type_keys(&mut s, "t");
    assert_eq!(
        screen(&mut s, 44, 10),
        [
            "  1   1 one                                 ",
            "  2     two                                 ",
            "      2 TWO                                 ",
            "  3   3 three                               ",
            "      4 inserted                            ",
            "  4   5 four                                ",
            "  5   6 five                                ",
            "                                            ",
            "                                            ",
            " src/demo.rs                2 changes   1/7 ",
        ]
        .join("\n")
    );
}

#[test]
fn reading_inline_gives_the_text_more_room_than_two_columns() {
    // One text column instead of two, which is why long lines need less
    // horizontal scrolling inline. The same 26-character line is cut off side
    // by side and complete inline, at the same terminal width.
    let line = "abcdefghijklmnopqrstuvwxyz";
    let mut s = session("long.rs", line, "abcdefghijklmnopqrstuvwxyZ");
    let columns = screen(&mut s, 44, 4);
    assert!(!columns.contains(line), "it fitted after all: {columns:?}");
    type_keys(&mut s, "t");
    let inline = screen(&mut s, 44, 4);
    assert!(inline.contains(line), "still cut off: {inline:?}");
}

#[test]
fn a_file_with_no_second_version_has_only_one_way_to_be_read() {
    // `t` is bound at the view level, so it reaches a single-file buffer too.
    // Nothing to lay out two ways, so it must be inert rather than an error.
    let mut s = TestSession::new(single("new.rs", "alpha\nbeta"), Theme::DARK);
    let before = screen(&mut s, 40, 4);
    type_keys(&mut s, "t");
    assert_eq!(screen(&mut s, 40, 4), before);
}
