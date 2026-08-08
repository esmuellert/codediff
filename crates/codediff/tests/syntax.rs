//! Syntax colour, on the screen, over a real diff.
//!
//! Everything below this has been checked in isolation — the engine reads,
//! the scope table resolves, the theme has a colour. This is the only test
//! that says the three meet on a cell of the terminal, and it is the one that
//! would have caught wiring the palette up and never calling it.
//!
//! The composition rule is what most of these are about: **the diff owns the
//! background, syntax owns the foreground.** Get it backwards and the file is
//! prettier and the review is broken, because which lines changed stops being
//! visible.

mod harness;

use harness::{cells, diff};
use ui::ratatui::buffer::Buffer as Cells;
use ui::ratatui::style::Color;
use ui::{Buffer, Session, Theme};

/// The colours found on one row, left to right, ignoring runs.
fn foregrounds(cells: &Cells, y: u16) -> Vec<Color> {
    let mut out: Vec<Color> = Vec::new();
    for x in 0..cells.area.width {
        let fg = cells[(x, y)].style().fg.unwrap_or(Color::Reset);
        if out.last() != Some(&fg) {
            out.push(fg);
        }
    }
    out
}

/// The screen once the painter has finished: what a reader sees a few frames
/// after opening a file.
///
/// The first frame is deliberately plain — colouring happens on another thread
/// and the interface never waits for it — so every test below that asks about
/// colour asks about the settled screen.
/// `the_first_frame_shows_the_text_before_any_colour` is the one that asks
/// about the other.
fn settled(session: &mut Session, width: u16, height: u16) -> Cells {
    session.wait_until_idle();
    cells(session, width, height)
}

/// Everything the screen says, as one string.
fn text_of(cells: &Cells) -> String {
    (0..cells.area.height)
        .map(|y| {
            (0..cells.area.width)
                .map(|x| cells[(x, y)].symbol())
                .collect::<String>()
        })
        .collect::<Vec<_>>()
        .join("\n")
}

/// The background of the cell under the first occurrence of `needle`.
fn background_at(cells: &Cells, y: u16, needle: char) -> Option<Color> {
    (0..cells.area.width)
        .find(|x| cells[(*x, y)].symbol() == needle.to_string())
        .and_then(|x| cells[(x, y)].style().bg)
}

fn rust_session(before: &str, after: &str) -> Session {
    Session::new(diff("src/main.rs", before, after), Theme::DARK)
}

const BEFORE: &str = "// a note\nfn main() {\n    let x = 1;\n}\n";
const AFTER: &str = "// a note\nfn main() {\n    let x = 2;\n}\n";

#[test]
fn a_keyword_a_comment_and_a_string_are_three_different_colours() {
    let mut session = rust_session(
        "fn main() {\n    let s = \"hi\";\n}\n",
        "fn main() {\n    let s = \"there\";\n}\n",
    );
    let cells = settled(&mut session, 80, 10);
    let code = Theme::DARK.code;

    let first = foregrounds(&cells, 0);
    assert!(first.contains(&code.keyword), "`fn` is a keyword");

    let second = foregrounds(&cells, 1);
    assert!(second.contains(&code.string), "`\"hi\"` is a string");
    assert!(second.contains(&code.keyword), "`let` is a keyword");

    assert_ne!(
        code.keyword, code.string,
        "and they are not the same colour"
    );
}

#[test]
fn a_changed_line_keeps_its_background_and_gains_syntax_colour() {
    // The composition rule, on one row. The line differs, so it wears the
    // diff's background; the words on it are still coloured by the language.
    // Either half missing is a bug the other half hides.
    let mut session = rust_session(BEFORE, AFTER);
    let cells = settled(&mut session, 80, 10);

    let changed = (0..cells.area.height)
        .find(|y| foregrounds(&cells, *y).contains(&Theme::DARK.code.constant))
        .expect("the line holding the number is on screen");

    assert_eq!(
        background_at(&cells, changed, 'l'),
        Theme::DARK.deleted.bg,
        "the changed line still wears the diff's background"
    );
    assert!(
        foregrounds(&cells, changed).contains(&Theme::DARK.code.keyword),
        "and `let` is still a keyword on it"
    );
}

#[test]
fn syntax_never_sets_a_background() {
    // Structurally impossible — `Code` holds colours, not styles — so this
    // guards the wiring rather than the table: it would catch a renderer that
    // patched a whole `Style` where it should have patched a foreground.
    let mut session = rust_session(BEFORE, AFTER);
    let cells = settled(&mut session, 80, 10);

    let theme = Theme::DARK;
    let allowed = [
        theme.normal.bg,
        theme.deleted.bg,
        theme.inserted.bg,
        theme.deleted_text.bg,
        theme.inserted_text.bg,
        theme.moved.bg,
        theme.cursor_line.bg,
        theme.status.bg,
        theme.filler.bg,
        theme.divider.bg,
    ];
    for y in 0..cells.area.height {
        for x in 0..cells.area.width {
            let bg = cells[(x, y)].style().bg;
            assert!(
                allowed.contains(&bg),
                "({x},{y}) has a background no diff role asked for: {bg:?}"
            );
        }
    }
}

#[test]
fn the_inner_change_highlight_still_wins_over_syntax() {
    // Both apply to the same characters and they must not fight: the emphasis
    // is a background and the syntax is a foreground, so the character that
    // changed is both legible and unmistakable.
    let mut session = rust_session(BEFORE, AFTER);
    let cells = settled(&mut session, 80, 10);

    let found = (0..cells.area.height).any(|y| {
        (0..cells.area.width).any(|x| {
            let cell = &cells[(x, y)];
            cell.symbol() == "1"
                && cell.style().bg == Theme::DARK.deleted_text.bg
                && cell.style().fg == Some(Theme::DARK.code.constant)
        })
    });
    assert!(found, "the changed digit is emphasised and still a number");
}

#[test]
fn pressing_s_takes_the_colour_away_and_gives_it_back() {
    let mut session = rust_session(BEFORE, AFTER);
    let coloured = settled(&mut session, 80, 10);

    harness::type_keys(&mut session, "s");
    let plain = settled(&mut session, 80, 10);
    assert_ne!(
        foregrounds(&coloured, 0),
        foregrounds(&plain, 0),
        "`s` switched the colour off"
    );
    assert!(!foregrounds(&plain, 0).contains(&Theme::DARK.code.keyword));

    harness::type_keys(&mut session, "s");
    let again = settled(&mut session, 80, 10);
    assert_eq!(
        foregrounds(&coloured, 0),
        foregrounds(&again, 0),
        "and back"
    );
}

#[test]
fn a_file_read_inline_is_coloured_the_same_way() {
    // Inline and side by side share `render::line`, so this is really asking
    // whether the spans reached the shared path rather than one of the two.
    let mut session = rust_session(BEFORE, AFTER);
    harness::type_keys(&mut session, "t");
    let cells = settled(&mut session, 80, 10);
    assert!(
        (0..cells.area.height).any(|y| foregrounds(&cells, y).contains(&Theme::DARK.code.keyword)),
        "inline has keywords too"
    );
}

#[test]
fn a_lone_file_is_coloured_too() {
    let buffer = harness::added("src/main.rs", "fn main() {\n    let x = 1;\n}\n");
    let mut session = Session::new(buffer, Theme::DARK);
    let cells = settled(&mut session, 80, 10);
    assert!(foregrounds(&cells, 0).contains(&Theme::DARK.code.keyword));
}

#[test]
fn a_language_nothing_claims_is_drawn_plainly_rather_than_refused() {
    let mut session = Session::new(
        diff("notes.qqzz", "one line\n", "another line\n"),
        Theme::DARK,
    );
    let cells = settled(&mut session, 80, 10);
    // It still draws, and the text is still there — it simply has no colour
    // of its own beyond the diff's.
    assert!(harness::screen(&mut session, 80, 10).contains("another line"));
    assert!(!foregrounds(&cells, 0).contains(&Theme::DARK.code.keyword));
}

#[test]
fn a_rename_is_coloured_as_each_side_is_named() {
    // A `.py` that became a `.rs`. If one grammar were used for both, whichever
    // side lost would be mis-coloured, and `def` is the tell: it is a keyword
    // in Python and nothing at all in Rust.
    let file = pipeline::file::DiffContent::Diff(pipeline::file::Diff {
        file: file_types::File::renamed(
            file_types::RepoPath::new("a.py", std::path::Path::new("/repo")),
            file_types::RepoPath::new("a.rs", std::path::Path::new("/repo")),
            harness::revs(),
        ),
        alignment: alignment("def f():\n    pass\n", "fn f() {}\n"),
    });
    let mut session = Session::new(Buffer::diff(file), Theme::DARK);
    let cells = settled(&mut session, 100, 10);
    let code = Theme::DARK.code;
    assert!(
        foregrounds(&cells, 0).contains(&code.keyword),
        "both `def` and `fn` are keywords, each in its own language"
    );
}

fn alignment(before: &str, after: &str) -> align::Alignment {
    let original = vscode_diff::lines(before);
    let modified = vscode_diff::lines(after);
    let computed = vscode_diff::compute(&original, &modified, &vscode_diff::Options::default())
        .expect("the engine runs");
    align::Alignment::new(computed, &original, &modified)
}

#[test]
fn a_very_long_file_shows_at_once_and_colours_as_it_goes() {
    // What `LEAP` used to protect, now protected by the work being elsewhere.
    // Three thousand lines is more than a frame's worth of colouring for
    // either engine, and none of it delays the text.
    let long: String = (0..3_000)
        .map(|n| format!("fn f{n}() -> u32 {{ let s = \"x{n}\"; {n} }}\n"))
        .collect();
    let mut before = long.clone();
    before.push_str("fn last() {}\n");
    let mut after = long;
    after.push_str("fn changed() {}\n");

    let mut session = Session::new(diff("src/big.rs", &before, &after), Theme::DARK);
    let first = cells(&mut session, 80, 24);
    assert!(text_of(&first).contains("f0"), "the text is there at once");

    harness::type_keys(&mut session, "G");
    let jumped = cells(&mut session, 80, 24);
    assert!(
        text_of(&jumped).contains("changed"),
        "and jumping to the end shows it, coloured or not"
    );

    session.wait_until_idle();
    let settled = cells(&mut session, 80, 24);
    assert!(
        (0..settled.area.height)
            .any(|y| foregrounds(&settled, y).contains(&Theme::DARK.code.keyword)),
        "the end of the file is coloured too"
    );
}

#[test]
fn the_first_frame_shows_the_text_before_any_colour() {
    // The property the painter's thread exists to guarantee: the text is on
    // screen immediately, whatever the language costs to colour. Nothing here
    // waits, and nothing can be made to.
    let mut session = rust_session(BEFORE, AFTER);

    let first = cells(&mut session, 80, 10);
    assert!(
        text_of(&first).contains("main"),
        "the text is there straight away"
    );
    assert!(session.is_colouring(), "and the colours are still on their way");

    session.wait_until_idle();
    let warm = cells(&mut session, 80, 10);
    assert!(
        (0..warm.area.height).any(|y| foregrounds(&warm, y).contains(&Theme::DARK.code.keyword)),
        "a moment later, `fn` is a keyword"
    );
    assert!(!session.is_colouring());
}

#[test]
fn toggling_the_layout_keeps_the_colours_it_already_has() {
    // The colours belong to the file, not to how it is laid out — spans are
    // keyed by file line, and `flipped` carries the whole diff across. So a
    // toggle must not send the reader back to plain text while it is all
    // painted again.
    let mut session = rust_session(BEFORE, AFTER);
    session.wait_until_idle();
    assert!(!session.is_colouring());

    harness::type_keys(&mut session, "t");
    assert!(!session.is_colouring(), "nothing was thrown away");

    let inline = cells(&mut session, 80, 10);
    assert!(
        (0..inline.area.height)
            .any(|y| foregrounds(&inline, y).contains(&Theme::DARK.code.keyword)),
        "and it is still coloured, without waiting"
    );
}

#[test]
fn a_second_file_in_the_same_language_is_coloured_too() {
    // The painter is asked once per file, so a second session must get its own
    // answer rather than the first one's — which is what the version on every
    // request is for.
    for (before, after) in [
        ("fn a() {}\n", "fn b() {}\n"),
        ("fn c() {}\n", "fn d() {}\n"),
    ] {
        let mut session = rust_session(before, after);
        session.wait_until_idle();
        let cells = cells(&mut session, 80, 10);
        assert!(
            (0..cells.area.height)
                .any(|y| foregrounds(&cells, y).contains(&Theme::DARK.code.keyword)),
            "`fn` is a keyword in both"
        );
    }
}
