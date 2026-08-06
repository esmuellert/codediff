//! One changed file, as one row will show it.
//!
//! One per *comparison a file is in* rather than one per file: a path edited
//! and then edited again is in two, because there are two different diffs to
//! review. Which comparison is not a field here — it is the [`Group`] this
//! sits in, so the two cannot disagree.
//!
//! Assembled outside this crate. How many lines a file gained is a question
//! only a backend can answer, and this crate is not allowed to ask one.
//!
//! [`Group`]: crate::Group

use file_types::{ChangeType, ChangedFile, Stats};

/// One changed file, as one row will show it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Entry {
    pub file: ChangedFile,
    /// Lines gained and lost, or `None` when nothing counted them — a binary
    /// file, or a backend that was not asked.
    pub stats: Option<Stats>,
}

impl Entry {
    pub fn new(file: ChangedFile) -> Self {
        Self { file, stats: None }
    }

    pub fn with_stats(mut self, stats: Stats) -> Self {
        self.stats = Some(stats);
        self
    }

    /// The path as the backend spelled it, which is what the tree is built
    /// from and what the rows are sorted by.
    pub fn path(&self) -> &str {
        self.file.path().as_str()
    }

    /// The short code shown at the right-hand end of the row.
    ///
    /// Git's letters where a [`ChangeType`] has one. It has six variants and
    /// git prints eight: a **copy** arrives here as `Moved` and shows `R`, and
    /// a **type change** as `Modified` and shows `M`. Both are deliberate —
    /// what a reviewer does about either is read the new content, which is
    /// what those letters already promise — but it means this is git's
    /// vocabulary rather than the whole of git's alphabet.
    pub fn status(&self) -> &'static str {
        match self.file.change() {
            ChangeType::Added => "A",
            ChangeType::Modified => "M",
            ChangeType::Deleted => "D",
            ChangeType::Moved => "R",
            ChangeType::Untracked => "??",
            ChangeType::Conflicted => "!",
        }
    }

    /// Where a moved file came from, for the row to show beside its new name.
    pub fn moved_from(&self) -> Option<&str> {
        if !self.file.is_moved() {
            return None;
        }
        self.file
            .file
            .on(file_types::DiffVersion::Original)
            .map(|path| path.as_str())
            .filter(|previous| *previous != self.path())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use file_types::{File, Oid, RepoPath, Revs};
    use std::path::Path;

    fn revs() -> Revs {
        Revs::worktree_against(Oid::new("b87b24c"))
    }

    fn at(relative: &str) -> RepoPath {
        RepoPath::new(relative, Path::new("/repo"))
    }

    #[test]
    fn the_letters_are_gits_own() {
        let modified = ChangedFile::new(File::unchanged_path(at("a.rs"), revs()), None);
        assert_eq!(Entry::new(modified).status(), "M");

        let untracked =
            ChangedFile::reported(File::added(at("new.rs"), revs()), ChangeType::Untracked);
        assert_eq!(Entry::new(untracked).status(), "??");
    }

    #[test]
    fn a_rename_says_where_it_came_from() {
        let moved = ChangedFile::new(File::renamed(at("old.rs"), at("new.rs"), revs()), Some(90));
        let entry = Entry::new(moved);
        assert_eq!(entry.moved_from(), Some("old.rs"));
        assert_eq!(entry.path(), "new.rs");
    }

    #[test]
    fn a_file_that_did_not_move_has_nowhere_to_have_come_from() {
        let modified = ChangedFile::new(File::unchanged_path(at("a.rs"), revs()), None);
        assert_eq!(Entry::new(modified).moved_from(), None);
    }
}
