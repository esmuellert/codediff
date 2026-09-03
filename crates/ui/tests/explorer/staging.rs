use std::collections::HashSet;
use std::path::Path;

use file_types::{File, RepoPath, Rev, Revs};
use ui::components::explorer::build::{Node, grouped_tree};

use super::common::file;

fn staged_revs() -> Revs {
    Revs::new(Rev::Commit(file_types::Oid::new("abc")), Rev::Index)
}

fn staged_file(path: &str) -> File {
    File::unchanged_path(RepoPath::new(path, Path::new("/repo")), staged_revs())
}

fn directories(files: &[File]) -> Vec<(RepoPath, Revs)> {
    grouped_tree(files, &HashSet::new())
        .into_iter()
        .filter_map(|node| match node {
            Node::Directory { path, revs, .. } => Some((path, revs)),
            _ => None,
        })
        .collect()
}

#[test]
fn an_unstaged_directory_carries_its_path_and_revisions() {
    let files = vec![file("src/a.rs"), file("src/b.rs")];

    let directories = directories(&files);

    assert_eq!(directories.len(), 1);
    assert_eq!(directories[0].0.as_str(), "src");
    assert_eq!(directories[0].0.root(), Path::new("/repo"));
    assert_eq!(directories[0].1, files[0].revs());
}

#[test]
fn a_staged_directory_carries_its_path_and_revisions() {
    let files = vec![staged_file("src/a.rs"), staged_file("src/b.rs")];

    let directories = directories(&files);

    assert_eq!(directories.len(), 1);
    assert_eq!(directories[0].0.as_str(), "src");
    assert_eq!(directories[0].1, staged_revs());
}

#[test]
fn identical_directories_in_each_group_have_different_revisions() {
    let files = vec![file("src/a.rs"), staged_file("src/b.rs")];

    let directories = directories(&files);

    assert_eq!(directories.len(), 2);
    assert_eq!(directories[0].0, directories[1].0);
    assert_ne!(directories[0].1, directories[1].1);
    assert_eq!(directories[0].1.after, Rev::Worktree);
    assert_eq!(directories[1].1.after, Rev::Index);
}

#[test]
fn a_collapsed_directory_carries_its_full_path() {
    let directories = directories(&[file("src/view/file.rs")]);

    assert_eq!(directories[0].0.as_str(), "src/view");
}
