//! The flat arrangement: one line per file, showing its whole path.
//!
//! ---
//!
//! **There is nothing here but an order.** No nodes, no directories, no
//! parents, nothing foldable. A flat list is a list of *paths*, and building a
//! tree in order to walk it back into one line per file would be building a
//! structure whose only distinguishing property is that it is not used — which
//! is what this codebase did, and the dead `draw/…/explorer/list.rs` that
//! resulted is what gave it away. See D69.
//!
//! **One group's files, and nothing about groups**, the same as [`Tree`]. A
//! heading is what an arrangement sits under, not part of one.
//!
//! [`Tree`]: super::Tree

use file_types::File;

use super::ViewLine;
use super::order;

/// One group's files, in the order VS Code lists them.
#[derive(Debug, Default)]
pub struct List {
    /// Places in the list the whole explorer holds, in the order they are
    /// drawn. Both the arrangement and the line lookup, because in a flat list
    /// they are the same thing.
    view_lines: Vec<usize>,
}

impl List {
    /// Orders `members`, which are places in `files`.
    ///
    /// A key per path built once, then a plain sort — see
    /// [`order`](super::order). The path is carried beside the key as the
    /// tie-break the key deliberately leaves out.
    pub fn build(files: &[File], members: &[usize]) -> Self {
        let mut keyed: Vec<(Vec<u8>, &str, usize)> = members
            .iter()
            .map(|&index| {
                let path = files[index].path().as_str();
                (order::path_key(path), path, index)
            })
            .collect();
        keyed.sort_by(|left, right| left.0.cmp(&right.0).then_with(|| left.1.cmp(right.1)));
        List {
            view_lines: keyed.into_iter().map(|(_, _, index)| index).collect(),
        }
    }

    /// Which file is on each line, in order.
    pub fn view_lines(&self) -> &[usize] {
        &self.view_lines
    }

    /// The file on a line, as a place in the explorer's list.
    pub fn file_on(&self, line: usize) -> Option<usize> {
        self.view_lines.get(line).copied()
    }

    /// What is on a line, as facts.
    ///
    /// The name is the whole path, because nothing above it says where the
    /// file is — which is the whole difference from the nested arrangement.
    pub fn view_line<'a>(&self, line: usize, files: &'a [File]) -> Option<ViewLine<'a>> {
        let file = files.get(self.file_on(line)?)?;
        Some(ViewLine::File {
            name: file.path().as_str(),
            file,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use file_types::{File, Oid, RepoPath, Revs};
    use std::path::Path;

    fn built(paths: &[&str]) -> (Vec<File>, List) {
        let files: Vec<File> = paths
            .iter()
            .map(|path| {
                File::unchanged_path(
                    RepoPath::new(*path, Path::new("/repo")),
                    Revs::worktree_against(Oid::new("b87b24c")),
                )
            })
            .collect();
        let members: Vec<usize> = (0..files.len()).collect();
        let list = List::build(&files, &members);
        (files, list)
    }

    fn names(files: &[File], list: &List) -> Vec<String> {
        (0..list.view_lines().len())
            .map(|line| match list.view_line(line, files) {
                Some(ViewLine::File { name, .. }) => name.to_owned(),
                _ => panic!("a flat list holds files and nothing else"),
            })
            .collect()
    }

    #[test]
    fn every_file_is_one_line_whatever_its_depth() {
        let (files, list) = built(&["a/b/c/deep.rs", "top.rs"]);
        assert_eq!(list.view_lines().len(), 2);
        assert_eq!(names(&files, &list), vec!["top.rs", "a/b/c/deep.rs"]);
    }

    #[test]
    fn a_shallower_file_comes_before_a_deeper_one() {
        // Within a shared prefix, VS Code's `comparePaths` runs out of
        // segments on one side and returns there. Sorting the paths as plain
        // strings gives the opposite, because `/` is below every letter.
        let (files, list) = built(&["nest/b/deep.txt", "nest/a.txt"]);
        assert_eq!(names(&files, &list), vec!["nest/a.txt", "nest/b/deep.txt"]);
    }
}
