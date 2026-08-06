//! The tree, against what the Neovim plugin draws for the same repository.
//!
//! The fixture is the one `cargo xtask fixture-repo` builds, and the expected
//! rows were captured by running the plugin headless over it. Anything that
//! is deliberately not the same is marked where it appears.

use explorer::{Entry, Explorer, Group, Groups, ViewMode};
use file_types::{ChangeType, ChangedFile, File, Oid, RepoPath, Rev, Revs, Stats};
use std::path::Path;

fn revs() -> Revs {
    Revs::worktree_against(Oid::new("b87b24c"))
}

fn at(relative: &str) -> RepoPath {
    RepoPath::new(relative, Path::new("/repo"))
}

fn modified(path: &str) -> Entry {
    Entry::new(ChangedFile::new(
        File::unchanged_path(at(path), revs()),
        None,
    ))
}

fn untracked(path: &str) -> Entry {
    Entry::new(ChangedFile::reported(
        File::added(at(path), revs()),
        ChangeType::Untracked,
    ))
}

/// The two comparisons `codediff` with no arguments produces.
fn unstaged(files: Vec<Entry>) -> Group {
    Group::new("Changes", Revs::new(Rev::Index, Rev::Worktree), files)
}

fn staged(files: Vec<Entry>) -> Group {
    Group::new(
        "Staged Changes",
        Revs::new(Rev::Commit(Oid::new("b87b24c")), Rev::Index),
        files,
    )
}

/// One group, as a comparison of two revisions produces.
fn only(files: Vec<Entry>) -> Groups {
    vec![unstaged(files)]
}

fn text(explorer: &Explorer) -> Vec<String> {
    explorer.rows().iter().map(|row| row.text()).collect()
}

/// Every changed file in the fixture repository, as the backend reports them.
///
/// `staged-then-edited.txt` appears **twice**, which is not a duplicate: git
/// reports it as `MM`, so there is an unstaged diff and a staged diff of the
/// same path, and one row could not show both.
fn fixture() -> Groups {
    let mut unstaged_files = Vec::new();
    for path in [
        "crlf.txt",
        "gains-a-line.txt",
        "modified.txt",
        "no-trailing-newline.txt",
        "picture.png",
        "staged-then-edited.txt",
        "with spaces.txt",
        "ünïcodé-ファイル.txt",
    ] {
        unstaged_files.push(modified(path));
    }
    unstaged_files.push(untracked("untracked.txt"));
    unstaged_files.push(untracked("untracked-dir/inside.txt"));

    let staged_files = vec![
        Entry::new(ChangedFile::new(
            File::deleted(at("deleted.txt"), revs()),
            None,
        )),
        Entry::new(ChangedFile::new(
            File::renamed(at("renamed-from.txt"), at("renamed-to.txt"), revs()),
            Some(100),
        )),
        modified("staged-then-edited.txt"),
    ];

    vec![unstaged(unstaged_files), staged(staged_files)]
}

/// The nested directories the fixture gained so that flattening has something
/// to collapse. Without a chain of single-child directories, a flattened tree
/// and an unflattened one are the same picture.
fn nested() -> Groups {
    only(vec![
        untracked("deep/only/one/chain/leaf.txt"),
        untracked("nest/a/one.txt"),
        untracked("nest/b/two.txt"),
        untracked("nest/b/three.txt"),
    ])
}

#[test]
fn the_tree_matches_what_the_plugin_draws() {
    let mut explorer = Explorer::new(fixture());
    explorer.set_stats(false);
    assert_eq!(
        text(&explorer),
        vec![
            "Changes (10)",
            "├ ▾ untracked-dir",
            "│ └ inside.txt ??",
            "├ crlf.txt M",
            "├ gains-a-line.txt M",
            "├ modified.txt M",
            "├ no-trailing-newline.txt M",
            "├ picture.png M",
            "├ staged-then-edited.txt M",
            "├ untracked.txt ??",
            "├ with spaces.txt M",
            "└ ünïcodé-ファイル.txt M",
            "Staged Changes (3)",
            "├ deleted.txt D",
            "├ renamed-to.txt ← renamed-from.txt R",
            "└ staged-then-edited.txt M",
        ]
    );
}

#[test]
fn a_chain_of_directories_with_no_choice_in_it_becomes_one_row() {
    let mut explorer = Explorer::new(nested());
    explorer.set_stats(false);
    let rows = text(&explorer);
    assert!(
        rows.contains(&"├ ▾ deep/only/one/chain".to_string()),
        "four directories, one row: {rows:#?}"
    );
    assert!(
        rows.contains(&"└ ▾ nest".to_string()),
        "two children, so nothing to collapse: {rows:#?}"
    );
}

#[test]
fn without_flattening_every_directory_is_its_own_row() {
    let mut explorer = Explorer::new(nested());
    explorer.set_stats(false);
    explorer.set_flatten(false);
    let rows = text(&explorer);
    assert!(rows.contains(&"├ ▾ deep".to_string()), "{rows:#?}");
    assert!(rows.contains(&"│ └ ▾ only".to_string()), "{rows:#?}");
}

#[test]
fn the_guides_of_a_deep_tree_are_the_plugins_guides() {
    let mut explorer = Explorer::new(nested());
    explorer.set_stats(false);
    assert_eq!(
        text(&explorer),
        vec![
            "Changes (4)",
            "├ ▾ deep/only/one/chain",
            "│ └ leaf.txt ??",
            "└ ▾ nest",
            "  ├ ▾ a",
            "  │ └ one.txt ??",
            "  └ ▾ b",
            "    ├ three.txt ??",
            "    └ two.txt ??",
        ]
    );
}

#[test]
fn a_flat_list_shows_whole_paths_in_the_order_vs_code_uses() {
    let mut explorer = Explorer::new(nested());
    explorer.set_stats(false);
    explorer.set_mode(ViewMode::List);
    assert_eq!(
        text(&explorer),
        vec![
            "Changes (4)",
            "├ deep/only/one/chain/leaf.txt ??",
            "├ nest/a/one.txt ??",
            "├ nest/b/three.txt ??",
            "└ nest/b/two.txt ??",
        ]
    );
}

#[test]
fn shutting_a_directory_hides_what_is_under_it_and_nothing_else() {
    let mut explorer = Explorer::new(nested());
    explorer.set_stats(false);
    let before = explorer.rows().len();

    // Row 3 is `nest`, which holds two directories and three files.
    explorer.select(3);
    assert!(explorer.toggle());
    let rows = text(&explorer);
    assert_eq!(rows.len(), before - 5);
    assert!(
        rows.contains(&"└ ▸ nest".to_string()),
        "and says it is shut"
    );
    assert!(
        rows.contains(&"│ └ leaf.txt ??".to_string()),
        "its neighbour is untouched: {rows:#?}"
    );

    assert!(explorer.toggle());
    assert_eq!(explorer.rows().len(), before, "and opening restores it");
}

#[test]
fn a_file_row_cannot_be_folded() {
    let mut explorer = Explorer::new(nested());
    explorer.select(2);
    assert_eq!(
        explorer.node().map(|node| node.name.clone()),
        Some("leaf.txt".into())
    );
    assert!(!explorer.toggle(), "there is nothing under it to hide");
}

#[test]
fn stats_are_shown_per_file_and_summed_on_the_heading() {
    let entries = vec![
        modified("a.rs").with_stats(Stats::new(4, 0)),
        modified("b.rs").with_stats(Stats::new(2, 3)),
    ];
    assert_eq!(
        text(&Explorer::new(only(entries))),
        vec!["Changes (2 · +6 -3)", "├ a.rs +4 M", "└ b.rs +2 -3 M"]
    );
}

#[test]
fn a_file_nothing_counted_shows_no_numbers() {
    // A picture: git reports `-` for both sides rather than a count, and
    // `+0 -0` beside it would claim a measurement nobody made.
    let entries = vec![modified("picture.png")];
    assert_eq!(
        text(&Explorer::new(only(entries))),
        vec!["Changes (1)", "└ picture.png M"]
    );
}

#[test]
fn a_pattern_hides_the_files_it_does_not_match() {
    let mut explorer = Explorer::new(nested());
    explorer.set_stats(false);
    explorer.set_pattern(Some("nest/b/*".into()));
    assert_eq!(
        text(&explorer),
        vec![
            "Changes (2)",
            "└ ▾ nest/b",
            "  ├ three.txt ??",
            "  └ two.txt ??",
        ],
        "and the directory above them collapses, having only one child left"
    );

    explorer.set_pattern(None);
    assert_eq!(explorer.rows().len(), 9, "and clearing it brings them back");
}

#[test]
fn a_new_explorer_starts_on_the_first_file() {
    // Row zero is a heading, which folds but cannot be opened, so a reader
    // starting there would press the open key and see nothing happen.
    // Row 0 is the heading and row 1 a directory; the first file is under it.
    let explorer = Explorer::new(fixture());
    assert_eq!(explorer.selected(), 2);
    assert_eq!(
        explorer.entry().map(|entry| entry.path().to_owned()),
        Some("untracked-dir/inside.txt".into())
    );
}

#[test]
fn the_selection_survives_a_change_that_removes_the_row_it_was_on() {
    // A fold cannot test this: you can only fold the row you are *on*, and a
    // folded directory's own row always survives, so the clamp never fires.
    // What removes rows from above the selection is a filter.
    let mut explorer = Explorer::new(nested());
    let last = explorer.rows().len() - 1;
    explorer.select(last);
    assert_eq!(explorer.selected(), last);

    explorer.set_pattern(Some("deep/**".into()));
    assert!(
        explorer.selected() < explorer.rows().len(),
        "selected {} of {} rows",
        explorer.selected(),
        explorer.rows().len()
    );
}

#[test]
fn switching_view_mode_keeps_the_reader_on_the_same_file() {
    // A row number means nothing across a rebuild: tree mode has directory
    // rows that list mode does not, so row 5 is a different file in each. The
    // reader is put back on the file they were on, by name.
    let mut explorer = Explorer::new(nested());
    explorer.select(5);
    let before = explorer.anchor(5).expect("a file");
    assert_eq!(before.path, "nest/a/one.txt");

    explorer.set_mode(ViewMode::List);
    let landing = explorer.row_of(&before).expect("still listed");
    assert_eq!(
        landing, 2,
        "and it has moved, so a row number would be wrong"
    );
    assert_eq!(
        explorer.rows()[landing].text(),
        "├ nest/a/one.txt ??",
        "the same file"
    );
    // And the anchor really names one file rather than merely resolving:
    // every row must give back its own row number.
    for row in 0..explorer.rows().len() {
        let Some(anchor) = explorer.anchor(row) else {
            continue;
        };
        assert_eq!(explorer.row_of(&anchor), Some(row), "row {row}");
    }
}

#[test]
fn an_anchor_tells_the_two_sections_holding_one_path_apart() {
    // `staged-then-edited.txt` is listed twice, once per comparison. An anchor
    // that carried only the path would put the reader on the first of them
    // whichever they had chosen.
    let explorer = Explorer::new(fixture());
    let rows: Vec<usize> = explorer
        .rows()
        .iter()
        .enumerate()
        .filter(|(_, row)| row.text().contains("staged-then-edited.txt"))
        .map(|(row, _)| row)
        .collect();
    assert_eq!(rows.len(), 2, "the file is listed twice");
    for row in rows {
        let anchor = explorer.anchor(row).expect("a file");
        assert_eq!(explorer.row_of(&anchor), Some(row), "row {row}");
    }
}

#[test]
fn a_file_a_filter_has_hidden_leaves_the_cursor_where_it_was() {
    let mut explorer = Explorer::new(nested());
    let anchor = explorer.anchor(2).expect("a file");
    explorer.set_pattern(Some("nest/b/*".into()));
    assert_eq!(explorer.row_of(&anchor), None, "it is not listed any more");
}
