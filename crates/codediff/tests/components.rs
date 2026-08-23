//! What the components draw, against the rows `screens.rs` already asserts.

use std::rc::Rc;

use loom::testing::Harness;
use ui::components::{
    CursorContext, Diffs, DiffsContext, FirstCellContext, Reading, SideBySide, SideBySideProps,
    ThemeContext, ViewLinesContext,
};
use ui::Theme;

const BEFORE: &str = "one\ntwo\nthree\nfour\nfive";
const AFTER: &str = "one\nTWO\nthree\ninserted\nfour\nfive";

fn revs() -> file_types::Revs {
    file_types::Revs::worktree_against(file_types::Oid::new("b87b24c"))
}

fn diff(path: &str, before: &str, after: &str) -> Rc<pipeline::file::Diff> {
    let original = vscode_diff::lines(before);
    let modified = vscode_diff::lines(after);
    let computed = vscode_diff::compute(&original, &modified, &vscode_diff::Options::default())
        .expect("the engine runs");
    Rc::new(pipeline::file::Diff {
        file: file_types::File::unchanged_path(
            file_types::RepoPath::new(path, std::path::Path::new("/repo")),
            revs(),
        ),
        alignment: align::Alignment::new(computed, &original, &modified),
    })
}

fn screen(width: u16, height: u16, view_lines: std::ops::Range<u32>) -> Vec<String> {
    let diffs = Diffs::new();
    diffs.fill(Reading {
        diff: Some(diff("src/demo.rs", BEFORE, AFTER)),
        colours: Rc::new(syntax::Store::new()),
        syntax_on: false,
    });

    let mut harness = Harness::new::<SideBySide>(SideBySideProps {}, width, height)
        .provide::<ThemeContext>(Rc::new(Theme::DARK))
        .provide::<DiffsContext>(diffs)
        .provide::<ViewLinesContext>(view_lines)
        .provide::<CursorContext>(0)
        .provide::<FirstCellContext>(0);
    harness.screen()
}

/// The first seven rows of `a_small_diff_side_by_side`, without its status
/// line.
#[test]
fn a_small_diff_side_by_side() {
    assert_eq!(
        screen(44, 7, 0..7),
        [
            "  1 one              │  1 one",
            "  2 two              │  2 TWO",
            "  3 three            │  3 three",
            "╱╱╱╱╱╱╱╱╱╱╱╱╱╱╱╱╱╱╱╱╱│  4 inserted",
            "  4 four             │  5 four",
            "  5 five             │  6 five",
            // Past the end of the document on both sides, and the divider
            // still runs down.
            "                     │",
        ]
    );
}

/// The left numbers skip 3→4 across the filler while the right run 3,4,5 —
/// which is only correct because both columns were built from one row list.
#[test]
fn the_two_sides_never_show_different_rows() {
    let rows = screen(44, 7, 0..7);
    assert!(rows[3].starts_with('╱'), "left filler: {:?}", rows[3]);
    assert!(rows[3].contains("4 inserted"), "{:?}", rows[3]);
    assert!(rows[4].contains("  4 four") && rows[4].contains("  5 four"));
}

/// The whole interface: a diff and the status line under it.
#[test]
fn the_root_draws_a_diff_and_a_status_line() {
    use ui::components::{App, AppProps, Session};

    let diffs = Diffs::new();
    diffs.fill(Reading {
        diff: Some(diff("src/demo.rs", BEFORE, AFTER)),
        colours: Rc::new(syntax::Store::new()),
        syntax_on: false,
    });

    let mut harness = Harness::new::<App>(
        AppProps {
            session: Session {
                theme: Rc::new(Theme::DARK),
                repo: None,
                diffs: diffs.clone(),
            },
        },
        44,
        8,
    );

    let rows = harness.screen();
    assert_eq!(rows[0], "  1 one              │  1 one");
    assert_eq!(rows[3], "╱╱╱╱╱╱╱╱╱╱╱╱╱╱╱╱╱╱╱╱╱│  4 inserted");
    assert!(rows[7].contains("changed files"), "{:?}", rows[7]);
}
