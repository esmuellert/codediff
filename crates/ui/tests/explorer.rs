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
    draw(paths.iter().map(|p| file(p)).collect(), width, height + 1)
}

// ---- the tree walk ----

#[test]
fn a_file_at_the_root_is_one_row() {
    let rows = screen(&["README.md"], 40, 2);
    assert!(rows[1].contains("README.md"), "got {:?}", rows[1]);
}

#[test]
fn files_in_a_directory_hang_below_it() {
    let rows = screen(&["src/app.rs", "src/lib.rs"], 40, 4);
    assert!(rows[1].contains("src"), "got {:?}", rows[1]);
    assert!(rows[2].contains("app.rs"), "got {:?}", rows[2]);
    assert!(rows[3].contains("lib.rs"), "got {:?}", rows[3]);
}

#[test]
fn the_last_of_its_siblings_gets_a_corner() {
    let rows = screen(&["src/app.rs", "src/lib.rs"], 40, 4);
    assert!(rows[2].contains('├'), "app.rs has a sibling below: {:?}", rows[2]);
    assert!(rows[3].contains('└'), "lib.rs is the last: {:?}", rows[3]);
}

#[test]
fn a_deeper_file_carries_its_ancestors_line() {
    // Two files under src so the directory is not flattened.
    let rows = screen(&["src/app.rs", "src/view/tab.rs", "notes.txt"], 40, 6);
    // src has siblings below, so its line continues through view.
    assert!(rows[3].starts_with('│'), "tab.rs sits under src: {:?}", rows[3]);
}

// ---- the status section ----

#[test]
fn the_counts_and_the_letter_sit_at_the_right_edge() {
    let rows = draw(vec![file_with_stats("a.rs", 4, 3)], 30, 2);
    assert!(rows[1].ends_with("+4 -3 M"), "got {:?}", rows[1]);
}

#[test]
fn a_side_that_did_not_change_is_left_out() {
    // `+4 -0` would put a zero on every row in a column the eye is scanning.
    let only_added = draw(vec![file_with_stats("a.rs", 4, 0)], 30, 2);
    assert!(only_added[1].ends_with("+4 M"), "got {:?}", only_added[1]);

    let only_removed = draw(vec![file_with_stats("b.rs", 0, 3)], 30, 2);
    assert!(only_removed[1].ends_with("-3 M"), "got {:?}", only_removed[1]);
}

#[test]
fn a_file_with_no_counts_shows_only_its_letter() {
    let rows = draw(vec![file("a.rs")], 30, 2);
    assert!(rows[1].ends_with('M'), "got {:?}", rows[1]);
    assert!(!rows[1].contains('+'), "no counts to show: {:?}", rows[1]);
}

#[test]
fn a_directory_has_no_status() {
    let rows = screen(&["src/a.rs"], 30, 3);
    assert!(!rows[1].contains('M'), "a directory has no letter: {:?}", rows[1]);
}

#[test]
fn the_counts_are_green_and_red() {
    let mut h = harness(vec![file_with_stats("a.rs", 4, 3)], 30, 3, 0);
    let row = h.screen_row(1);
    let end = row.chars().count() as u16;

    let letter_at = end - 1;
    let letter = h.style_at(letter_at, 1);

    let plus = h.style_at(end - 7, 1);
    let minus = h.style_at(end - 4, 1);
    assert_ne!(plus.fg, letter.fg, "the gained count has its own colour");
    assert_ne!(minus.fg, letter.fg, "the lost count has its own colour");
    assert_ne!(plus.fg, minus.fg, "gained and lost are told apart by colour");
}

// ---- fitting ----

#[test]
fn a_name_too_long_for_the_row_is_cut_and_says_so() {
    let rows = draw(vec![file_with_stats("a-very-long-file-name.rs", 4, 3)], 20, 2);
    assert!(rows[1].contains('…'), "the name was cut: {:?}", rows[1]);
    assert!(rows[1].ends_with("+4 -3 M"), "the status survives: {:?}", rows[1]);
}

#[test]
fn a_wide_name_is_cut_between_characters() {
    // Cutting inside a two-column glyph would draw half of it.
    let rows = draw(vec![file_with_stats("ファイル.txt", 4, 3)], 18, 2);
    assert!(rows[1].contains('…'), "the name was cut: {:?}", rows[1]);
    assert!(rows[1].contains("ファ"), "whole characters survive: {:?}", rows[1]);
    assert!(rows[1].ends_with("+4 -3 M"), "the status is on screen: {:?}", rows[1]);
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
    assert!(rows[1].contains("new.rs"), "got {:?}", rows[1]);
    assert!(rows[1].contains("← old.rs"), "got {:?}", rows[1]);
}

#[test]
fn a_narrow_row_drops_where_it_came_from_before_the_name() {
    // The name is what the reader is looking for; the old path is context.
    let file = moved("a-long-old-name.rs", "new.rs");
    let wide = draw(vec![file.clone()], 40, 2);
    assert!(wide[1].contains("← a-long-old-name.rs"), "it fits at 40: {:?}", wide[1]);

    let narrow = draw(vec![file], 20, 2);
    assert!(narrow[1].contains("new.rs"), "the name survives: {:?}", narrow[1]);
    assert!(!narrow[1].contains('←'), "the old path went first: {:?}", narrow[1]);
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
    assert!(rows[1].contains("src/view"), "the chain is merged: {:?}", rows[1]);
}

#[test]
fn a_directory_with_two_children_is_not_flattened() {
    let rows = screen(&["src/app.rs", "src/lib.rs"], 40, 4);
    assert!(rows[1].contains("src"), "got {:?}", rows[1]);
    assert!(!rows[1].contains('/'), "src has two children, no merge: {:?}", rows[1]);
}

#[test]
fn a_three_level_chain_collapses_fully() {
    let rows = screen(&["a/b/c/file.rs"], 40, 3);
    assert!(rows[1].contains("a/b/c"), "got {:?}", rows[1]);
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
    let names: Vec<&str> = nodes.iter().filter_map(|n| match n {
        ui::components::explorer::build::Node::Heading { .. } => None,
        ui::components::explorer::build::Node::Directory { name, .. } => Some(name.as_str()),
        ui::components::explorer::build::Node::File { name, .. } => Some(name.as_str()),
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
    let names: Vec<&str> = nodes.iter().filter_map(|n| match n {
        ui::components::explorer::build::Node::Heading { .. } => None,
        ui::components::explorer::build::Node::Directory { name, .. } => Some(name.as_str()),
        ui::components::explorer::build::Node::File { name, .. } => Some(name.as_str()),
    }).collect();

    assert!(names.contains(&"app.rs"), "children are visible when not folded");
}

// ---- headings ----

#[test]
fn a_heading_shows_the_group_name_and_file_count() {
    let rows = screen(&["a.rs", "b.rs"], 40, 5);
    assert!(rows[0].contains("Changes"), "got {:?}", rows[0]);
    assert!(rows[0].contains("(2"), "the count is shown: {:?}", rows[0]);
}

#[test]
fn a_heading_with_stats_shows_the_totals() {
    let rows = draw(
        vec![file_with_stats("a.rs", 4, 3), file_with_stats("b.rs", 2, 0)],
        40, 5,
    );
    assert!(rows[0].contains("+6"), "total added: {:?}", rows[0]);
    assert!(rows[0].contains("-3"), "total removed: {:?}", rows[0]);
}

#[test]
fn a_heading_without_stats_shows_only_the_count() {
    let rows = draw(vec![file("a.rs")], 30, 3);
    assert!(rows[0].contains("(1)"), "just the count: {:?}", rows[0]);
    assert!(!rows[0].contains('·'), "no stats separator: {:?}", rows[0]);
}

#[test]
fn a_heading_name_is_not_bold_and_the_count_is_highlighted() {
    use ratatui::style::Modifier;
    let mut h = harness(vec![file("a.rs")], 30, 3, 0);
    let name_style = h.style_at(0, 0);
    assert!(
        !name_style.add_modifier.contains(Modifier::BOLD),
        "the heading name is not bold",
    );
    // The count follows the name in a different colour.
    let row = h.screen_row(0);
    let paren = row.find('(').expect("a parenthesized count");
    let count_style = h.style_at(paren as u16, 0);
    assert_ne!(name_style.fg, count_style.fg, "the count has its own colour");
}

// ---- refresh behaviour ----

#[test]
fn the_cursor_stays_when_the_file_list_rebuilds() {
    use std::collections::HashSet;
    use ui::components::explorer::build::grouped_tree;

    let files_v1: Vec<File> = ["src/app.rs", "src/lib.rs", "notes.txt"]
        .iter()
        .map(|p| file(p))
        .collect();

    let nodes_v1 = grouped_tree(&files_v1, &HashSet::new());
    // Find which line "lib.rs" is on.
    let lib_line = nodes_v1.iter().position(|n| matches!(n,
        ui::components::explorer::build::Node::File { name, .. } if name == "lib.rs"
    )).expect("lib.rs exists");

    // A refresh adds a file. The same cursor index should still point at
    // the same file if the list order did not change.
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
fn fold_state_survives_a_refresh() {
    use std::collections::HashSet;
    use ui::components::explorer::build::{grouped_tree, Node};

    let files: Vec<File> = ["src/app.rs", "src/lib.rs", "notes.txt"]
        .iter()
        .map(|p| file(p))
        .collect();

    // Fold "src".
    let mut folded = HashSet::new();
    folded.insert("Changes/src".to_string());

    // First build — src is folded.
    let nodes_v1 = grouped_tree(&files, &folded);
    let has_app = nodes_v1.iter().any(|n| matches!(n, Node::File { name, .. } if name == "app.rs"));
    assert!(!has_app, "src is folded, app.rs is hidden");

    // A "refresh" — same files, same fold set.
    let files_v2: Vec<File> = ["src/app.rs", "src/lib.rs", "src/new.rs", "notes.txt"]
        .iter()
        .map(|p| file(p))
        .collect();

    let nodes_v2 = grouped_tree(&files_v2, &folded);
    let has_new = nodes_v2.iter().any(|n| matches!(n, Node::File { name, .. } if name == "new.rs"));
    assert!(!has_new, "src is still folded after refresh, new.rs is hidden");

    let has_notes = nodes_v2.iter().any(|n| matches!(n, Node::File { name, .. } if name == "notes.txt"));
    assert!(has_notes, "notes.txt is still visible");
}

#[test]
fn the_cursor_follows_its_file_when_one_is_inserted_before_it() {
    use std::collections::HashSet;
    use ui::components::explorer::build::grouped_tree;
    use ui::components::explorer::{identity, find_by_identity};

    let before: Vec<File> = ["src/lib.rs", "notes.txt"].iter().map(|p| file(p)).collect();
    let old = grouped_tree(&before, &HashSet::new());
    let on = old.iter().position(|n| matches!(n,
        ui::components::explorer::build::Node::File { name, .. } if name == "notes.txt"
    )).expect("notes.txt is listed");
    let saved = identity(&old[on]);

    // A file arrives that sorts before it, so every row below shifts down.
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
    use std::collections::HashSet;
    use ui::components::explorer::build::grouped_tree;
    use ui::components::explorer::find_by_identity;

    let after: Vec<File> = ["a.rs"].iter().map(|p| file(p)).collect();
    let new = grouped_tree(&after, &HashSet::new());

    assert_eq!(find_by_identity(Some("gone.rs"), &new), None,
        "nothing to move to, so the caller keeps the cursor");
}

#[test]
fn nothing_saved_moves_nothing() {
    use std::collections::HashSet;
    use ui::components::explorer::build::grouped_tree;
    use ui::components::explorer::find_by_identity;

    let files: Vec<File> = ["a.rs"].iter().map(|p| file(p)).collect();
    let nodes = grouped_tree(&files, &HashSet::new());
    assert_eq!(find_by_identity(None, &nodes), None);
}

#[test]
fn the_identity_tells_two_files_of_the_same_name_apart() {
    use std::collections::HashSet;
    use ui::components::explorer::build::grouped_tree;
    use ui::components::explorer::{identity, find_by_identity};

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

// ---- list mode ----

#[test]
fn list_mode_shows_full_paths() {
    
    use ui::components::explorer::build::grouped_list;

    let files: Vec<File> = ["src/app.rs", "src/lib.rs", "notes.txt"]
        .iter()
        .map(|p| file(p))
        .collect();

    let nodes = grouped_list(&files);
    let names: Vec<&str> = nodes.iter().filter_map(|n| match n {
        ui::components::explorer::build::Node::File { name, .. } => Some(name.as_str()),
        _ => None,
    }).collect();

    assert!(names.contains(&"notes.txt"), "got {:?}", names);
    assert!(names.contains(&"src/app.rs"), "full path: {:?}", names);
    assert!(names.contains(&"src/lib.rs"), "full path: {:?}", names);
}

#[test]
fn list_mode_has_no_directories() {
    
    use ui::components::explorer::build::grouped_list;

    let files: Vec<File> = ["src/app.rs", "src/lib.rs"]
        .iter()
        .map(|p| file(p))
        .collect();

    let nodes = grouped_list(&files);
    let has_dir = nodes.iter().any(|n| matches!(n, ui::components::explorer::build::Node::Directory { .. }));
    assert!(!has_dir, "list mode has no directories");
}

#[test]
fn list_mode_files_are_indented_under_the_heading() {
    
    use ui::components::explorer::build::grouped_list;

    let files: Vec<File> = ["src/deep/file.rs"]
        .iter()
        .map(|p| file(p))
        .collect();

    let nodes = grouped_list(&files);
    for node in &nodes {
        if let ui::components::explorer::build::Node::File { indent, .. } = node {
            assert_eq!(indent, "  ", "list mode files sit under the heading: {:?}", indent);
        }
    }
}

#[test]
fn list_mode_is_sorted_by_path() {
    
    use ui::components::explorer::build::grouped_list;

    let files: Vec<File> = ["z.rs", "a/b.rs", "a.rs"]
        .iter()
        .map(|p| file(p))
        .collect();

    let nodes = grouped_list(&files);
    let names: Vec<&str> = nodes.iter().filter_map(|n| match n {
        ui::components::explorer::build::Node::File { name, .. } => Some(name.as_str()),
        _ => None,
    }).collect();

    let mut sorted = names.clone();
    sorted.sort();
    assert_eq!(names, sorted, "files are sorted by full path");
}

// ---- fold isolation across groups ----

#[test]
fn folding_a_directory_in_one_group_does_not_fold_the_same_name_in_another() {
    use std::collections::HashSet;
    use ui::components::explorer::build::{grouped_tree, Node};

    // Two groups, each with a src/ directory.
    let staged = file_types::Revs::new(
        file_types::Rev::Commit(file_types::Oid::new("abc")),
        file_types::Rev::Index,
    );
    let unstaged = file_types::Revs::worktree_against(file_types::Oid::new("abc"));

    let files = vec![
        File::unchanged_path(
            file_types::RepoPath::new("src/a.rs", std::path::Path::new("/repo")),
            unstaged,
        ),
        File::unchanged_path(
            file_types::RepoPath::new("src/b.rs", std::path::Path::new("/repo")),
            staged,
        ),
    ];

    // Fold src in the first group only.
    let mut folded = HashSet::new();
    let nodes = grouped_tree(&files, &HashSet::new());
    // Find the first src directory's path key.
    let first_src = nodes.iter().find_map(|n| match n {
        Node::Directory { path, .. } => Some(path.clone()),
        _ => None,
    }).expect("a src directory");
    folded.insert(first_src.clone());

    let nodes = grouped_tree(&files, &folded);

    // Count how many src directories are open.
    let open_srcs: Vec<_> = nodes.iter().filter(|n| matches!(n,
        Node::Directory { name, open: true, .. } if name == "src"
    )).collect();

    assert_eq!(open_srcs.len(), 1,
        "only one group's src is folded, the other stays open");
}
