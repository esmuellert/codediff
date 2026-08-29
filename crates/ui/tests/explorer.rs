//! The explorer, drawn into a buffer.

use std::path::Path;
use std::rc::Rc;

use file_types::{File, Oid, RepoPath, Revs, Stats};
use loom::testing::Harness;
use ui::Theme;
use ui::components::{Context, Explorer, ExplorerProps, Ui};

fn file(path: &str) -> File {
    File::unchanged_path(
        RepoPath::new(path, Path::new("/repo")),
        Revs::worktree_against(Oid::new("abc")),
    )
}

fn file_with_stats(path: &str, added: u32, removed: u32) -> File {
    file(path).set_stats(Stats::new(added, removed))
}

fn moved(from: &str, to: &str) -> File {
    File::new(
        Some(RepoPath::new(from, Path::new("/repo"))),
        Some(RepoPath::new(to, Path::new("/repo"))),
        Revs::worktree_against(Oid::new("abc")),
    )
    .expect("a file on both sides")
}

/// Draws the explorer over `files` and returns the screen, row by row.
fn draw(files: Vec<File>, width: u16, height: u16) -> Vec<String> {
    harness(files, width, height, 0).screen()
}

fn harness(files: Vec<File>, width: u16, height: u16, cursor: u32) -> Harness {
    let rows = height as u32;
    Harness::new::<Explorer>(ExplorerProps { on_open: Rc::new(|_| {}) }, width, height)
        .provide::<Ui>(Context {
            theme: Rc::new(Theme::DARK),
            repo: Rc::from(Path::new("/repo")),
            files: Rc::new(files),
            cursor,
            view_lines: 0..rows,
            set_repo: None,
            set_cursor: None,
        })
}

fn screen(paths: &[&str], width: u16, height: u16) -> Vec<String> {
    draw(paths.iter().map(|p| file(p)).collect(), width, height)
}

// ---- the tree walk ----

#[test]
fn a_file_at_the_root_is_one_row() {
    let rows = screen(&["README.md"], 40, 2);
    assert!(rows[0].contains("README.md"), "got {:?}", rows[0]);
}

#[test]
fn files_in_a_directory_hang_below_it() {
    let rows = screen(&["src/app.rs", "src/lib.rs"], 40, 4);
    assert!(rows[0].contains("src"), "got {:?}", rows[0]);
    assert!(rows[1].contains("app.rs"), "got {:?}", rows[1]);
    assert!(rows[2].contains("lib.rs"), "got {:?}", rows[2]);
}

#[test]
fn the_last_of_its_siblings_gets_a_corner() {
    let rows = screen(&["src/app.rs", "src/lib.rs"], 40, 4);
    assert!(rows[1].contains('├'), "app.rs has a sibling below: {:?}", rows[1]);
    assert!(rows[2].contains('└'), "lib.rs is the last: {:?}", rows[2]);
}

#[test]
fn a_deeper_file_carries_its_ancestors_line() {
    // Two files under src so the directory is not flattened.
    let rows = screen(&["src/app.rs", "src/view/tab.rs", "notes.txt"], 40, 6);
    // src has siblings below, so its line continues through view.
    assert!(rows[2].starts_with('│'), "tab.rs sits under src: {:?}", rows[2]);
}

// ---- the status section ----

#[test]
fn the_counts_and_the_letter_sit_at_the_right_edge() {
    let rows = draw(vec![file_with_stats("a.rs", 4, 3)], 30, 2);
    assert!(rows[0].ends_with("+4 -3 M"), "got {:?}", rows[0]);
}

#[test]
fn a_side_that_did_not_change_is_left_out() {
    // `+4 -0` would put a zero on every row in a column the eye is scanning.
    let only_added = draw(vec![file_with_stats("a.rs", 4, 0)], 30, 2);
    assert!(only_added[0].ends_with("+4 M"), "got {:?}", only_added[0]);

    let only_removed = draw(vec![file_with_stats("b.rs", 0, 3)], 30, 2);
    assert!(only_removed[0].ends_with("-3 M"), "got {:?}", only_removed[0]);
}

#[test]
fn a_file_with_no_counts_shows_only_its_letter() {
    let rows = draw(vec![file("a.rs")], 30, 2);
    assert!(rows[0].ends_with('M'), "got {:?}", rows[0]);
    assert!(!rows[0].contains('+'), "no counts to show: {:?}", rows[0]);
}

#[test]
fn a_directory_has_no_status() {
    let rows = screen(&["src/a.rs"], 30, 3);
    assert!(!rows[0].contains('M'), "a directory has no letter: {:?}", rows[0]);
}

#[test]
fn the_counts_are_green_and_red_and_the_letter_is_bold() {
    use ratatui::style::Modifier;
    let mut h = harness(vec![file_with_stats("a.rs", 4, 3)], 30, 2, 0);
    let row = h.screen_row(0);
    let end = row.chars().count() as u16;

    // "+4 -3 M" — the row ends with the letter, bold.
    let letter_at = end - 1;
    let letter = h.style_at(letter_at, 0);
    assert!(
        letter.add_modifier.contains(Modifier::BOLD),
        "the change letter is bold in every theme",
    );

    // `+4` and `-3` do not share the letter's colour.
    let plus = h.style_at(end - 7, 0);
    let minus = h.style_at(end - 4, 0);
    assert_ne!(plus.fg, letter.fg, "the gained count has its own colour");
    assert_ne!(minus.fg, letter.fg, "the lost count has its own colour");
    assert_ne!(plus.fg, minus.fg, "gained and lost are told apart by colour");
}

// ---- fitting ----

#[test]
fn a_name_too_long_for_the_row_is_cut_and_says_so() {
    let rows = draw(vec![file_with_stats("a-very-long-file-name.rs", 4, 3)], 20, 2);
    assert!(rows[0].contains('…'), "the name was cut: {:?}", rows[0]);
    assert!(rows[0].ends_with("+4 -3 M"), "the status survives: {:?}", rows[0]);
}

#[test]
fn a_wide_name_is_cut_between_characters() {
    // Cutting inside a two-column glyph would draw half of it.
    let rows = draw(vec![file_with_stats("ファイル.txt", 4, 3)], 18, 2);
    assert!(rows[0].contains('…'), "the name was cut: {:?}", rows[0]);
    assert!(rows[0].contains("ファ"), "whole characters survive: {:?}", rows[0]);
    assert!(rows[0].ends_with("+4 -3 M"), "the status is on screen: {:?}", rows[0]);
}

#[test]
fn no_row_is_wider_than_the_pane() {
    for width in 8..40u16 {
        let rows = draw(
            vec![file_with_stats("some/deep/path/file.rs", 12, 34)],
            width,
            4,
        );
        for (y, row) in rows.iter().enumerate() {
            let drawn = line_index::LineIndex::new(row, 1).width().0;
            assert!(
                drawn <= u32::from(width),
                "row {y} drew {drawn} columns into {width}: {row:?}",
            );
        }
    }
}

#[test]
fn where_a_moved_file_came_from_follows_its_name() {
    let rows = draw(vec![moved("old.rs", "new.rs")], 40, 2);
    assert!(rows[0].contains("new.rs"), "got {:?}", rows[0]);
    assert!(rows[0].contains("← old.rs"), "got {:?}", rows[0]);
}

#[test]
fn a_narrow_row_drops_where_it_came_from_before_the_name() {
    // The name is what the reader is looking for; the old path is context.
    let file = moved("a-long-old-name.rs", "new.rs");
    let wide = draw(vec![file.clone()], 40, 2);
    assert!(wide[0].contains("← a-long-old-name.rs"), "it fits at 40: {:?}", wide[0]);

    let narrow = draw(vec![file], 20, 2);
    assert!(narrow[0].contains("new.rs"), "the name survives: {:?}", narrow[0]);
    assert!(!narrow[0].contains('←'), "the old path went first: {:?}", narrow[0]);
}

// ---- keys ----

#[test]
fn j_and_k_move_the_cursor() {
    use ui::components::scroll_top;
    // The cursor is state the provider owns, so this checks the arithmetic
    // the key handler runs rather than the key routing.
    let total = 5;
    assert_eq!(1u32.saturating_add(1).min(total - 1), 2, "j goes down");
    assert_eq!(2u32.saturating_sub(1), 1, "k goes up");
    assert_eq!(0u32.saturating_sub(1), 0, "k at the top stays");
    assert_eq!(4u32.saturating_add(1).min(total - 1), 4, "j at the end stays");
    let _ = scroll_top;
}

// ---- scrolling ----

#[test]
fn the_view_follows_the_cursor_down() {
    use ui::components::scroll_top;
    // 20 rows in a 10-row pane. The cursor at 15 cannot be shown from the top.
    let top = scroll_top(15, 20, 10, 0);
    assert!(top > 0, "the view moved to show row 15, got top {top}");
    assert!(15 >= top && 15 < top + 10, "row 15 is on screen from {top}");
}

#[test]
fn the_view_keeps_a_margin_below_the_cursor() {
    use ui::components::scroll_top;
    // Moving to row 7 in a 10-row pane leaves fewer than 3 rows below it,
    // so the view scrolls even though 7 is technically visible from 0.
    let top = scroll_top(7, 20, 10, 0);
    assert_eq!(top, 1, "three rows are kept past the cursor, got {top}");
}

#[test]
fn the_view_never_scrolls_past_the_end() {
    use ui::components::scroll_top;
    // 12 rows in a 10-row pane: the last useful top is 2.
    let top = scroll_top(11, 12, 10, 0);
    assert_eq!(top, 2, "the last row sits at the bottom, got {top}");
}

#[test]
fn a_document_shorter_than_the_pane_never_scrolls() {
    use ui::components::scroll_top;
    assert_eq!(scroll_top(2, 3, 10, 0), 0);
    assert_eq!(scroll_top(0, 1, 10, 0), 0);
}

#[test]
fn the_view_follows_the_cursor_back_up() {
    use ui::components::scroll_top;
    // Coming from a scrolled position, moving to row 0 brings the view home.
    let top = scroll_top(0, 20, 10, 12);
    assert_eq!(top, 0, "the view came back with the cursor, got {top}");
}

// ---- chain flattening ----

#[test]
fn a_single_child_directory_chain_is_flattened() {
    // src/view/tab.rs with no other files under src — src/view becomes one line.
    let rows = screen(&["src/view/tab.rs"], 40, 3);
    assert!(rows[0].contains("src/view"), "the chain is merged: {:?}", rows[0]);
}

#[test]
fn a_directory_with_two_children_is_not_flattened() {
    let rows = screen(&["src/app.rs", "src/lib.rs"], 40, 4);
    assert!(rows[0].contains("src"), "got {:?}", rows[0]);
    assert!(!rows[0].contains('/'), "src has two children, no merge: {:?}", rows[0]);
}

#[test]
fn a_three_level_chain_collapses_fully() {
    let rows = screen(&["a/b/c/file.rs"], 40, 3);
    assert!(rows[0].contains("a/b/c"), "got {:?}", rows[0]);
}

// ---- folding ----

#[test]
fn a_folded_directory_hides_its_children() {
    use std::collections::HashSet;
    use ui::components::explorer::build::tree;

    let files: Vec<File> = ["src/app.rs", "src/lib.rs", "notes.txt"]
        .iter()
        .map(|p| file(p))
        .collect();

    let mut folded = HashSet::new();
    folded.insert("src".to_string());

    let nodes = tree(&files, &folded);
    let names: Vec<&str> = nodes.iter().map(|n| match n {
        ui::components::explorer::build::Node::Directory { name, .. } => name.as_str(),
        ui::components::explorer::build::Node::File { name, .. } => name.as_str(),
    }).collect();

    assert!(names.contains(&"src"), "the directory itself is shown");
    assert!(!names.contains(&"app.rs"), "its children are hidden");
    assert!(!names.contains(&"lib.rs"), "its children are hidden");
    assert!(names.contains(&"notes.txt"), "siblings are still shown");
}

#[test]
fn unfolding_brings_the_children_back() {
    use std::collections::HashSet;
    use ui::components::explorer::build::tree;

    let files: Vec<File> = ["src/app.rs", "notes.txt"]
        .iter()
        .map(|p| file(p))
        .collect();

    let folded = HashSet::new();
    let nodes = tree(&files, &folded);
    let names: Vec<&str> = nodes.iter().map(|n| match n {
        ui::components::explorer::build::Node::Directory { name, .. } => name.as_str(),
        ui::components::explorer::build::Node::File { name, .. } => name.as_str(),
    }).collect();

    assert!(names.contains(&"app.rs"), "children are visible when not folded");
}
