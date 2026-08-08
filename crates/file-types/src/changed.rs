//! One changed file, as a version control system reports it.
//!
//! What a backend must produce, and therefore what everything downstream
//! receives — whether the backend is git, jj, or something not written yet.
//! Nothing here names a version control concept: no index, no `HEAD`, no blob
//! and no object id, because a system need not have any of them. jj has no
//! staging area at all. What "before" means is decided when a backend is
//! constructed, not here. See D30.

use crate::{ChangeType, File, RepoPath, Stats};

/// One file that differs between the two sides.
///
/// Identity is a [`File`], which every layer above can name. What happened is
/// **not stored** where the paths already say it: `Added`, `Deleted`, `Moved`
/// and `Modified` are read from the pair by [`File::change`], so no field here
/// can contradict them. Only what a backend alone knows is kept.
///
/// Git's rename *score* — how alike the two paths were, which is how git
/// decided it was a rename at all — is not carried. Nothing a reader can see
/// showed it, and a field with no way to reach it is not a fact this layer
/// holds. `vcs` still parses it, because git prints it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ChangedFile {
    /// Where the file is on each side. Absent on one side means added or
    /// deleted, and two different paths mean a rename.
    pub file: File,
    /// What the backend reported, when the paths cannot say it.
    ///
    /// `Some` only for [`ChangeType::Untracked`] and
    /// [`ChangeType::Conflicted`]: an untracked file's paths look exactly like
    /// an added one's, and a conflicted file's look like an ordinary
    /// modification.
    reported: Option<ChangeType>,
    /// Lines gained and lost, or `None` when nothing counted them — a binary
    /// file, or a backend that was not asked.
    ///
    /// Counting is a second question from listing, and a backend that will not
    /// answer it loses the numbers rather than the whole list. So this arrives
    /// after the file does, through [`with_stats`](Self::with_stats).
    pub stats: Option<Stats>,
}

impl ChangedFile {
    /// A file whose paths tell the whole story.
    pub fn new(file: File) -> Self {
        Self {
            file,
            reported: None,
            stats: None,
        }
    }

    /// A file the backend has more to say about than its paths do.
    ///
    /// # Panics
    ///
    /// If `reported` is one the paths could have said. Passing `Added` here
    /// would create exactly the disagreement this type exists to prevent.
    pub fn reported(file: File, reported: ChangeType) -> Self {
        assert!(
            reported.needs_a_backend(),
            "{reported:?} is readable from the paths; do not store it"
        );
        Self {
            file,
            reported: Some(reported),
            stats: None,
        }
    }

    /// The same file, with what it gained and lost.
    pub fn with_stats(mut self, stats: Stats) -> Self {
        self.stats = Some(stats);
        self
    }

    /// What happened to this file.
    pub fn change(&self) -> ChangeType {
        self.reported.unwrap_or_else(|| self.file.change())
    }

    /// Where the file is now, or where it was if it is gone.
    pub fn path(&self) -> &RepoPath {
        self.file.path()
    }

    pub fn is_conflicted(&self) -> bool {
        self.change() == ChangeType::Conflicted
    }

    pub fn is_moved(&self) -> bool {
        self.change() == ChangeType::Moved
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::Path;

    fn at(relative: &str) -> RepoPath {
        RepoPath::new(relative, Path::new("/repo"))
    }

    /// The ordinary comparison. Which revisions these are is not what any
    /// test below is about, so it is said once.
    fn revs() -> crate::Revs {
        crate::Revs::worktree_against(crate::Oid::new("b87b24c"))
    }

    #[test]
    fn the_ordinary_cases_come_from_the_paths() {
        assert_eq!(
            ChangedFile::new(File::added(at("new.rs"), revs())).change(),
            ChangeType::Added
        );
        assert_eq!(
            ChangedFile::new(File::renamed(at("o.rs"), at("n.rs"), revs())).change(),
            ChangeType::Moved
        );
    }

    #[test]
    fn the_backend_supplies_only_what_the_paths_cannot_say() {
        let untracked =
            ChangedFile::reported(File::added(at("new.rs"), revs()), ChangeType::Untracked);
        assert_eq!(untracked.change(), ChangeType::Untracked);
        assert_eq!(
            untracked.file.change(),
            ChangeType::Added,
            "the paths still say what they say"
        );
    }

    #[test]
    #[should_panic(expected = "readable from the paths")]
    fn storing_a_derivable_change_is_refused() {
        // The whole point: a stored `Added` could disagree with a `File` that
        // has both versions, and nothing would catch it.
        ChangedFile::reported(File::added(at("new.rs"), revs()), ChangeType::Added);
    }
}
