//! Both shapes, against what the Neovim plugin draws for the same repository.
//!
//! The fixture is the one `cargo xtask fixture-repo` builds, and the expected
//! lines were captured by running the plugin headless over it. Anything that is
//! deliberately not the same is marked where it appears.
//!
//! [`spell`] below is this file's own. A line is facts — a heading, a
//! directory, or a file — and the characters live in `draw`, beside the theme
//! that colours them. These tests are about which lines exist and in what
//! order, so they spell that the way a reader recognises it. What is really
//! drawn is asserted in `explorer_rows.rs`, against a screen.

use file_types::{ChangeType, File, Oid, RepoPath, Rev, Revs, Stats};
use std::path::Path;
use ui::view::buffer::explorer::{Explorer, ViewLine, ViewMode};

/// One line's facts, in the form these tests read them by.
///
/// The indent is only asked of the nested shape, because it is the only one
/// that has ancestors to describe.
fn spell(explorer: &Explorer, line: u32) -> String {
    let mut out = String::new();
    let view_line = explorer.view_line(line).expect("a line");

    if let ViewLine::Heading { name, files, stats } = &view_line {
        out.push_str(&format!("{name} ({files}"));
        if !stats.is_empty() {
            out.push_str(" · ");
            if stats.added > 0 {
                out.push_str(&format!("+{}", stats.added));
            }
            if stats.removed > 0 {
                let separator = if stats.added > 0 { " " } else { "" };
                out.push_str(&format!("{separator}-{}", stats.removed));
            }
        }
        out.push(')');
        return out;
    }

    out.push_str(&indent(explorer, line));

    match view_line {
        ViewLine::Heading { .. } => unreachable!("answered above"),
        ViewLine::Directory { name, open } => {
            out.push_str(if open { "▾ " } else { "▸ " });
            out.push_str(name);
        }
        ViewLine::File { name, file } => {
            out.push_str(name);
            if let Some(previous) = file.previous_path() {
                out.push_str(&format!(" ← {previous}"));
            }
            if let Some(stats) = file.get_stats().filter(|s| !s.is_empty()) {
                if stats.added > 0 {
                    out.push_str(&format!(" +{}", stats.added));
                }
                if stats.removed > 0 {
                    out.push_str(&format!(" -{}", stats.removed));
                }
            }
            out.push(' ');
            out.push_str(match file.get_change_type() {
                ChangeType::Added => "A",
                ChangeType::Modified => "M",
                ChangeType::Deleted => "D",
                ChangeType::Moved => "R",
                ChangeType::Untracked => "??",
                ChangeType::Conflicted => "!",
            });
        }
    }
    out
}

/// The guide columns before a line, read off the node's own parents.
///
/// Empty in the flat shape, which has no ancestors — and is what
/// `explorer_rows.rs` checks against a screen.
fn indent(explorer: &Explorer, line: u32) -> String {
    let Some((tree, id)) = explorer.nested_at(line) else {
        return String::new();
    };
    let node = tree.node(id);
    let mut levels = vec![if node.is_last { "└ " } else { "├ " }];
    let mut above = node.parent;
    while let Some(parent) = above {
        let parent = tree.node(parent);
        levels.push(if parent.is_last { "  " } else { "│ " });
        above = parent.parent;
    }
    levels.into_iter().rev().collect()
}

/// Every line, as facts.
fn text(explorer: &Explorer) -> Vec<String> {
    (0..explorer.view_lines())
        .map(|line| spell(explorer, line))
        .collect()
}

fn revs() -> Revs {
    Revs::new(Rev::Index, Rev::Worktree)
}

fn staged_revs() -> Revs {
    Revs::new(Rev::Commit(Oid::new("b87b24c")), Rev::Index)
}

fn at(relative: &str) -> RepoPath {
    RepoPath::new(relative, Path::new("/repo"))
}

fn modified(path: &str) -> File {
    File::unchanged_path(at(path), revs())
}

fn untracked(path: &str) -> File {
    File::added(at(path), revs()).set_change_type(ChangeType::Untracked)
}

/// Every changed file in the fixture repository, as the backend reports them.
///
/// `staged-then-edited.txt` appears twice, which is not a duplicate: git
/// reports it as `MM`, so there is an unstaged diff and a staged diff of the
/// same path, and one row could not show both. The two are told apart by the
/// revisions they carry, which is also what puts them in different groups.
fn fixture() -> Vec<File> {
    let mut files = Vec::new();
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
        files.push(modified(path));
    }
    files.push(untracked("untracked.txt"));
    files.push(untracked("untracked-dir/inside.txt"));

    files.push(File::deleted(at("deleted.txt"), staged_revs()));
    files.push(File::renamed(
        at("renamed-from.txt"),
        at("renamed-to.txt"),
        staged_revs(),
    ));
    files.push(File::unchanged_path(
        at("staged-then-edited.txt"),
        staged_revs(),
    ));
    files
}

/// The nested directories the fixture gained so that flattening has something
/// to collapse. Without a chain of single-child directories, a flattened tree
/// and an unflattened one are the same picture.
fn nested() -> Vec<File> {
    vec![
        untracked("deep/only/one/chain/leaf.txt"),
        untracked("nest/a/one.txt"),
        untracked("nest/b/two.txt"),
        untracked("nest/b/three.txt"),
    ]
}

#[test]
fn the_tree_matches_what_the_plugin_draws() {
    assert_eq!(
        text(&Explorer::new(fixture())),
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
    let rows = text(&Explorer::new(nested()));
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
fn the_guides_of_a_deep_tree_are_the_plugins_guides() {
    assert_eq!(
        text(&Explorer::new(nested())),
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
    explorer.set_mode(ViewMode::List);
    assert_eq!(
        text(&explorer),
        vec![
            "Changes (4)",
            // No guides: a flat list is a list of paths, and a guide beside
            // one would draw a tree where there is none. VS Code's list mode
            // draws none either. See D69.
            "deep/only/one/chain/leaf.txt ??",
            "nest/a/one.txt ??",
            "nest/b/three.txt ??",
            "nest/b/two.txt ??",
        ]
    );
}

#[test]
fn a_flat_list_puts_a_shallower_file_before_a_deeper_one() {
    // The rule `nested()` cannot show, because its paths differ at their first
    // segment and so never reach it. Within a shared prefix, VS Code's
    // `comparePaths` runs out of segments on one side and returns there —
    // where sorting the paths as plain strings gives the opposite answer,
    // since `/` is below every letter.
    let mut explorer = Explorer::new(vec![untracked("nest/b/deep.txt"), untracked("nest/a.txt")]);
    explorer.set_mode(ViewMode::List);
    assert_eq!(
        text(&explorer),
        vec!["Changes (2)", "nest/a.txt ??", "nest/b/deep.txt ??"]
    );
}

#[test]
fn shutting_a_directory_hides_what_is_under_it_and_nothing_else() {
    let mut explorer = Explorer::new(nested());
    let before = explorer.view_lines() as usize;

    // Row 3 is `nest`, which holds two directories and three files.
    assert!(explorer.toggle(3));
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

    assert!(explorer.toggle(3));
    assert_eq!(
        explorer.view_lines() as usize,
        before,
        "and opening restores it"
    );
}

#[test]
fn a_file_row_cannot_be_folded() {
    let mut explorer = Explorer::new(nested());
    assert!(
        matches!(explorer.view_line(2), Some(ViewLine::File { name, .. }) if name == "leaf.txt")
    );
    assert!(!explorer.toggle(2), "there is nothing under it to hide");
}

#[test]
fn stats_are_shown_per_file_and_summed_on_the_heading() {
    let files = vec![
        modified("a.rs").set_stats(Stats::new(4, 0)),
        modified("b.rs").set_stats(Stats::new(2, 3)),
    ];
    assert_eq!(
        text(&Explorer::new(files)),
        vec!["Changes (2 · +6 -3)", "├ a.rs +4 M", "└ b.rs +2 -3 M"]
    );
}

#[test]
fn a_file_nothing_counted_shows_no_numbers() {
    // A picture: git reports `-` for both sides rather than a count, and
    // `+0 -0` beside it would claim a measurement nobody made.
    assert_eq!(
        text(&Explorer::new(vec![modified("picture.png")])),
        vec!["Changes (1)", "└ picture.png M"]
    );
}

#[test]
fn a_pattern_hides_the_files_it_does_not_match() {
    let mut explorer = Explorer::new(nested());
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
    assert_eq!(
        explorer.view_lines() as usize,
        9,
        "and clearing it brings them back"
    );
}

#[test]
fn the_list_starts_on_the_first_file() {
    // Row zero is a heading, which folds but cannot be opened, so a reader
    // starting there would press the open key and see nothing happen. Row 1
    // is a directory; the first file is under it.
    let explorer = Explorer::new(fixture());
    assert_eq!(explorer.first_file(), 2);
    assert_eq!(
        explorer.file(2).map(|file| file.path().as_str().to_owned()),
        Some("untracked-dir/inside.txt".into())
    );
}

#[test]
fn the_selection_survives_a_change_that_removes_the_row_it_was_on() {
    // A fold cannot test this: you can only fold the row you are *on*, and a
    // folded directory's own row always survives, so the clamp never fires.
    // What removes rows from above the selection is a filter.
    let mut explorer = Explorer::new(nested());
    let last = explorer.view_lines() - 1;
    let landing = explorer.reshape_around(last, |model| {
        model.set_pattern(Some("deep/**".into()));
    });
    assert!(
        landing < explorer.view_lines(),
        "landed on {landing} of {} rows",
        explorer.view_lines() as usize
    );
}

#[test]
fn switching_view_mode_keeps_the_reader_on_the_same_file() {
    // A row number means nothing across a rebuild: tree mode has directory
    // rows that list mode does not, so row 5 is a different file in each. The
    // reader is put back on the file they were on, by name. See D54.
    let mut explorer = Explorer::new(nested());
    assert_eq!(
        explorer.file(5).map(|f| f.path().as_str().to_owned()),
        Some("nest/a/one.txt".into())
    );

    let landing = explorer.reshape_around(5, |model| model.set_mode(ViewMode::List));
    assert_eq!(
        landing, 2,
        "and it has moved, so a row number would have been wrong"
    );
    assert_eq!(
        spell(&explorer, landing),
        "nest/a/one.txt ??",
        "the same file"
    );

    // And the anchor really names one file rather than merely resolving:
    // every file row must give back its own row number.
    for row in 0..explorer.view_lines() as usize as u32 {
        if explorer.file(row).is_none() {
            continue;
        }
        assert_eq!(explorer.reshape_around(row, |_| {}), row, "row {row}");
    }
}

#[test]
fn an_anchor_tells_the_two_groups_holding_one_path_apart() {
    // `staged-then-edited.txt` is listed twice, once per comparison. An anchor
    // that carried only the path would put the reader on the first of them
    // whichever they had chosen — which is why it carries the revisions too.
    let mut explorer = Explorer::new(fixture());
    let rows: Vec<u32> = (0..explorer.view_lines() as usize as u32)
        .filter(|&row| {
            explorer
                .file(row)
                .is_some_and(|file| file.path().as_str() == "staged-then-edited.txt")
        })
        .collect();
    assert_eq!(rows.len(), 2, "the file is listed twice");
    for row in rows {
        assert_eq!(explorer.reshape_around(row, |_| {}), row, "row {row}");
    }
}

#[test]
fn a_file_a_filter_has_hidden_leaves_the_cursor_where_it_was() {
    let mut explorer = Explorer::new(nested());
    // Row 2 is `deep/.../leaf.txt`, which the pattern below hides.
    let landing = explorer.reshape_around(2, |model| {
        model.set_pattern(Some("nest/b/*".into()));
    });
    assert!(
        explorer
            .file(landing)
            .is_none_or(|file| file.path().as_str() != "deep/only/one/chain/leaf.txt"),
        "it is not listed any more"
    );
}
