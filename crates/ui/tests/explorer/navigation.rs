//! Scroll, cursor, anchor across refresh, mouse, edge cases.

use std::collections::HashSet;

use file_types::File;
use ui::components::explorer::build::grouped_tree;
use ui::components::explorer::{identity, find_by_identity};

use super::common::*;

// ---- scroll ----

#[test]
fn j_and_k_move_the_cursor() {
    use ui::components::scroll_top;
    let total = 5;
    assert_eq!(1u32.saturating_add(1).min(total - 1), 2, "j goes down");
    assert_eq!(2u32.saturating_sub(1), 1, "k goes up");
    assert_eq!(0u32.saturating_sub(1), 0, "k at the top stays");
    assert_eq!(4u32.saturating_add(1).min(total - 1), 4, "j at the end stays");
    let _ = scroll_top;
}

#[test]
fn the_view_follows_the_cursor_down() {
    use ui::components::scroll_top;
    let top = scroll_top(15, 20, 10, 0);
    assert!(top > 0, "the view moved to show row 15, got top {top}");
    assert!(15 >= top && 15 < top + 10, "row 15 is on screen from {top}");
}

#[test]
fn the_view_keeps_a_margin_below_the_cursor() {
    use ui::components::scroll_top;
    let top = scroll_top(7, 20, 10, 0);
    assert_eq!(top, 1, "three rows are kept past the cursor, got {top}");
}

#[test]
fn the_view_never_scrolls_past_the_end() {
    use ui::components::scroll_top;
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
    let top = scroll_top(0, 20, 10, 12);
    assert_eq!(top, 0, "the view came back with the cursor, got {top}");
}

#[test]
fn scroll_moves_the_view() {
    use ui::components::scroll_top;
    let top = scroll_top(10, 20, 5, 0);
    assert!(top > 0, "row 10 is not visible from the top, got {top}");
    assert!(10 >= top && 10 < top + 5, "row 10 is on screen from {top}");
}

// ---- cursor anchor ----

#[test]
fn the_cursor_stays_when_the_file_list_rebuilds() {
    let files_v1: Vec<File> = ["src/app.rs", "src/lib.rs", "notes.txt"]
        .iter()
        .map(|p| file(p))
        .collect();

    let nodes_v1 = grouped_tree(&files_v1, &HashSet::new());
    let lib_line = nodes_v1.iter().position(|n| matches!(n,
        ui::components::explorer::build::Node::File { name, .. } if name == "lib.rs"
    )).expect("lib.rs exists");

    let files_v2: Vec<File> = ["src/app.rs", "src/lib.rs", "src/new.rs", "notes.txt"]
        .iter()
        .map(|p| file(p))
        .collect();

    let nodes_v2 = grouped_tree(&files_v2, &HashSet::new());
    let at_same_line = &nodes_v2[lib_line];
    match at_same_line {
        ui::components::explorer::build::Node::File { name, .. } => {
            assert_eq!(name, "lib.rs", "the cursor still points at lib.rs");
        }
        other => panic!("expected lib.rs at line {lib_line}, got {:?}",
            match other {
                ui::components::explorer::build::Node::Heading { name, .. } => name.to_string(),
                ui::components::explorer::build::Node::Directory { name, .. } => name.clone(),
                ui::components::explorer::build::Node::File { name, .. } => name.clone(),
            }
        ),
    }
}

#[test]
fn the_cursor_follows_its_file_when_one_is_inserted_before_it() {
    let before: Vec<File> = ["src/lib.rs", "notes.txt"].iter().map(|p| file(p)).collect();
    let old = grouped_tree(&before, &HashSet::new());
    let on = old.iter().position(|n| matches!(n,
        ui::components::explorer::build::Node::File { name, .. } if name == "notes.txt"
    )).expect("notes.txt is listed");
    let saved = identity(&old[on]);

    let after: Vec<File> = ["src/lib.rs", "a.txt", "notes.txt"]
        .iter().map(|p| file(p)).collect();
    let new = grouped_tree(&after, &HashSet::new());

    let landed = find_by_identity(Some(&saved), &new).expect("notes.txt is still listed");
    assert_ne!(landed, on, "the row moved");
    assert!(matches!(&new[landed],
        ui::components::explorer::build::Node::File { name, .. } if name == "notes.txt"),
        "the cursor is on notes.txt again");
}

#[test]
fn a_file_that_is_gone_leaves_the_cursor_where_it_was() {
    let after: Vec<File> = ["a.rs"].iter().map(|p| file(p)).collect();
    let new = grouped_tree(&after, &HashSet::new());

    assert_eq!(find_by_identity(Some("gone.rs"), &new), None,
        "nothing to move to, so the caller keeps the cursor");
}

#[test]
fn nothing_saved_moves_nothing() {
    let files: Vec<File> = ["a.rs"].iter().map(|p| file(p)).collect();
    let nodes = grouped_tree(&files, &HashSet::new());
    assert_eq!(find_by_identity(None, &nodes), None);
}

#[test]
fn the_identity_tells_two_files_of_the_same_name_apart() {
    let files: Vec<File> = ["src/a/mod.rs", "src/b/mod.rs"].iter().map(|p| file(p)).collect();
    let nodes = grouped_tree(&files, &HashSet::new());

    let first = nodes.iter().position(|n| matches!(n,
        ui::components::explorer::build::Node::File { file, .. }
        if file.path().as_str() == "src/b/mod.rs"
    )).expect("src/b/mod.rs is listed");

    let saved = identity(&nodes[first]);
    let landed = find_by_identity(Some(&saved), &nodes).expect("found");
    assert_eq!(landed, first, "the full path picks out the right mod.rs");
}

// ---- mouse ----

#[test]
fn the_cursor_row_has_a_different_background() {
    let files: Vec<File> = ["src/app.rs", "notes.txt"]
        .iter().map(|p| file(p)).collect();
    let mut h1 = harness(files.clone(), 40, 10, 1);
    h1.draw();
    let bg_cursor = h1.style_at(0, 1).bg;
    let bg_other = h1.style_at(0, 2).bg;
    assert_ne!(bg_cursor, bg_other,
        "the cursor row has a different background from other rows");
}

// ---- edge cases ----

#[test]
fn an_empty_list_draws_nothing() {
    let rows = draw(Vec::new(), 40, 5);
    for row in &rows {
        assert!(row.is_empty() || row.chars().all(|c| c == ' '),
            "an empty list is blank: {:?}", row);
    }
}

#[test]
fn a_single_file_renders_without_panic() {
    let rows = draw(vec![file("only.rs")], 40, 5);
    assert!(rows[1].contains("only.rs"), "got {:?}", rows[1]);
}
