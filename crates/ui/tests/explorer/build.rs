//! Tree walk, flattening, folding, groups, list mode, headings.

use std::collections::HashSet;

use file_types::File;
use ui::components::explorer::build::{grouped_list, grouped_tree, Node};

use super::common::*;

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

// ---- chain flattening ----

#[test]
fn a_single_child_directory_chain_is_flattened() {
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
    let files: Vec<File> = ["src/app.rs", "src/lib.rs", "notes.txt"]
        .iter()
        .map(|p| file(p))
        .collect();

    let mut folded = HashSet::new();
    folded.insert("Changes/src".to_string());

    let nodes = grouped_tree(&files, &folded);
    let names: Vec<&str> = nodes.iter().filter_map(|n| match n {
        Node::Heading { .. } => None,
        Node::Directory { name, .. } => Some(name.as_str()),
        Node::File { name, .. } => Some(name.as_str()),
    }).collect();

    assert!(names.contains(&"src"), "the directory itself is shown");
    assert!(!names.contains(&"app.rs"), "its children are hidden");
    assert!(!names.contains(&"lib.rs"), "its children are hidden");
    assert!(names.contains(&"notes.txt"), "siblings are still shown");
}

#[test]
fn unfolding_brings_the_children_back() {
    let files: Vec<File> = ["src/app.rs", "notes.txt"]
        .iter()
        .map(|p| file(p))
        .collect();

    let folded = HashSet::new();
    let nodes = grouped_tree(&files, &folded);
    let names: Vec<&str> = nodes.iter().filter_map(|n| match n {
        Node::Heading { .. } => None,
        Node::Directory { name, .. } => Some(name.as_str()),
        Node::File { name, .. } => Some(name.as_str()),
    }).collect();

    assert!(names.contains(&"app.rs"), "children are visible when not folded");
}

#[test]
fn fold_state_survives_a_refresh() {
    let files: Vec<File> = ["src/app.rs", "src/lib.rs", "notes.txt"]
        .iter()
        .map(|p| file(p))
        .collect();

    let mut folded = HashSet::new();
    folded.insert("Changes/src".to_string());

    let nodes_v1 = grouped_tree(&files, &folded);
    let has_app = nodes_v1.iter().any(|n| matches!(n, Node::File { name, .. } if name == "app.rs"));
    assert!(!has_app, "src is folded, app.rs is hidden");

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
fn folding_a_directory_in_one_group_does_not_fold_the_same_name_in_another() {
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

    let mut folded = HashSet::new();
    let nodes = grouped_tree(&files, &HashSet::new());
    let first_src = nodes.iter().find_map(|n| match n {
        Node::Directory { path, .. } => Some(path.clone()),
        _ => None,
    }).expect("a src directory");
    folded.insert(first_src.clone());

    let nodes = grouped_tree(&files, &folded);

    let open_srcs: Vec<_> = nodes.iter().filter(|n| matches!(n,
        Node::Directory { name, open: true, .. } if name == "src"
    )).collect();

    assert_eq!(open_srcs.len(), 1,
        "only one group's src is folded, the other stays open");
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

// ---- list mode ----

#[test]
fn list_mode_shows_full_paths() {
    let files: Vec<File> = ["src/app.rs", "src/lib.rs", "notes.txt"]
        .iter()
        .map(|p| file(p))
        .collect();

    let nodes = grouped_list(&files);
    let names: Vec<&str> = nodes.iter().filter_map(|n| match n {
        Node::File { name, .. } => Some(name.as_str()),
        _ => None,
    }).collect();

    assert!(names.contains(&"notes.txt"), "got {:?}", names);
    assert!(names.contains(&"src/app.rs"), "full path: {:?}", names);
    assert!(names.contains(&"src/lib.rs"), "full path: {:?}", names);
}

#[test]
fn list_mode_has_no_directories() {
    let files: Vec<File> = ["src/app.rs", "src/lib.rs"]
        .iter()
        .map(|p| file(p))
        .collect();

    let nodes = grouped_list(&files);
    let has_dir = nodes.iter().any(|n| matches!(n, Node::Directory { .. }));
    assert!(!has_dir, "list mode has no directories");
}

#[test]
fn list_mode_files_are_indented_under_the_heading() {
    let files: Vec<File> = ["src/deep/file.rs"]
        .iter()
        .map(|p| file(p))
        .collect();

    let nodes = grouped_list(&files);
    for node in &nodes {
        if let Node::File { indent, .. } = node {
            assert_eq!(indent, "  ", "list mode files sit under the heading: {:?}", indent);
        }
    }
}

#[test]
fn list_mode_is_sorted_by_path() {
    let files: Vec<File> = ["z.rs", "a/b.rs", "a.rs"]
        .iter()
        .map(|p| file(p))
        .collect();

    let nodes = grouped_list(&files);
    let names: Vec<&str> = nodes.iter().filter_map(|n| match n {
        Node::File { name, .. } => Some(name.as_str()),
        _ => None,
    }).collect();

    let mut sorted = names.clone();
    sorted.sort();
    assert_eq!(names, sorted, "files are sorted by full path");
}
