//! A file under review: its path, revisions, and change type.

use crate::{DiffVersion, RepoPath, Rev, Stats};

/// What happened between two versions of a file.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ChangeType {
    /// Exists only as the modified version.
    Added,
    /// Exists as both, with different content.
    Modified,
    /// Exists only as the original version.
    Deleted,
    /// The same file under a different path.
    Moved,
    /// Not under version control at all, so there is no original.
    Untracked,
    /// Left unresolved by a merge.
    Conflicted,
}

impl ChangeType {
    /// True when only one version exists, so there is nothing to pair against.
    pub fn is_one_sided(self) -> bool {
        matches!(
            self,
            ChangeType::Added | ChangeType::Deleted | ChangeType::Untracked
        )
    }

    /// Whether this type cannot be inferred from paths.
    pub fn needs_a_backend(self) -> bool {
        matches!(self, ChangeType::Untracked | ChangeType::Conflicted)
    }
}

/// A file's paths and revisions on both sides of a review.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct File {
    original: Option<RepoPath>,
    modified: Option<RepoPath>,
    before: Rev,
    after: Rev,
    /// Backend-only change information.
    changed_type: Option<ChangeType>,
    /// Lines gained and lost, when counted.
    stats: Option<Stats>,
}

/// Revisions read for both sides of a file.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Revs {
    pub before: Rev,
    pub after: Rev,
}

impl Revs {
    pub fn new(before: Rev, after: Rev) -> Self {
        Self { before, after }
    }

    /// A commit compared with the working tree.
    pub fn worktree_against(commit: crate::Oid) -> Self {
        Self::new(Rev::Commit(commit), Rev::Worktree)
    }

    /// The heading text for this comparison (e.g. "Staged Changes").
    /// Derived from the revision pair.
    pub fn heading(&self) -> &'static str {
        match self.after {
            Rev::Index => "Staged Changes",
            _ => "Changes",
        }
    }
}

/// Neither side exists, which is not a file.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Nowhere;

impl std::fmt::Display for Nowhere {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("a file must exist on at least one side")
    }
}

impl std::error::Error for Nowhere {}

impl File {
    /// A file present on both sides, under one path.
    pub fn unchanged_path(path: RepoPath, revs: Revs) -> Self {
        Self {
            original: Some(path.clone()),
            modified: Some(path),
            before: revs.before,
            after: revs.after,
            changed_type: None,
            stats: None,
        }
    }

    /// A file that moved, or whose content changed under a new name.
    pub fn renamed(original: RepoPath, modified: RepoPath, revs: Revs) -> Self {
        Self {
            original: Some(original),
            modified: Some(modified),
            before: revs.before,
            after: revs.after,
            changed_type: None,
            stats: None,
        }
    }

    /// A file that does not exist on the original side.
    pub fn added(path: RepoPath, revs: Revs) -> Self {
        Self {
            original: None,
            modified: Some(path),
            before: revs.before,
            after: revs.after,
            changed_type: None,
            stats: None,
        }
    }

    /// A file that does not exist on the modified side.
    pub fn deleted(path: RepoPath, revs: Revs) -> Self {
        Self {
            original: Some(path),
            modified: None,
            before: revs.before,
            after: revs.after,
            changed_type: None,
            stats: None,
        }
    }

    /// Builds a file that exists on at least one side.
    pub fn new(
        original: Option<RepoPath>,
        modified: Option<RepoPath>,
        revs: Revs,
    ) -> Result<Self, Nowhere> {
        if original.is_none() && modified.is_none() {
            return Err(Nowhere);
        }
        Ok(Self {
            original,
            modified,
            before: revs.before,
            after: revs.after,
            changed_type: None,
            stats: None,
        })
    }

    /// Where the file is on one side, or `None` if it is not there.
    pub fn path_of_version(&self, version: DiffVersion) -> Option<&RepoPath> {
        match version {
            DiffVersion::Original => self.original.as_ref(),
            DiffVersion::Modified => self.modified.as_ref(),
        }
    }

    /// The revision inspected for one side.
    pub fn rev(&self, version: DiffVersion) -> &Rev {
        match version {
            DiffVersion::Original => &self.before,
            DiffVersion::Modified => &self.after,
        }
    }

    /// A stable name for one side's content, when it exists.
    pub fn name(&self, version: DiffVersion) -> Option<String> {
        let path = self.path_of_version(version)?;
        Some(match self.rev(version).stored() {
            Some(rev) => format!("{rev}:{path}"),
            None => format!("worktree:{path}"),
        })
    }

    /// The only side present, or `None` when both exist.
    pub fn is_one_sided(&self) -> Option<DiffVersion> {
        match (&self.original, &self.modified) {
            (None, Some(_)) => Some(DiffVersion::Modified),
            (Some(_), None) => Some(DiffVersion::Original),
            _ => None,
        }
    }

    /// Both revisions as one value.
    pub fn revs(&self) -> Revs {
        Revs::new(self.before.clone(), self.after.clone())
    }

    /// Whether the file has different paths on the two sides.
    pub fn is_renamed(&self) -> bool {
        match (&self.original, &self.modified) {
            (Some(before), Some(after)) => before != after,
            _ => false,
        }
    }

    /// The current path, or the previous path for a deletion.
    pub fn path(&self) -> &RepoPath {
        self.modified
            .as_ref()
            .or(self.original.as_ref())
            .expect("a file exists on at least one side")
    }

    /// The backend's change type, or the type inferred from paths.
    pub fn get_change_type(&self) -> ChangeType {
        self.changed_type
            .unwrap_or_else(|| self.change_type_of_paths())
    }

    /// The change type inferred only from paths.
    pub fn change_type_of_paths(&self) -> ChangeType {
        match (self.is_one_sided(), self.is_renamed()) {
            (Some(crate::DiffVersion::Modified), _) => ChangeType::Added,
            (Some(crate::DiffVersion::Original), _) => ChangeType::Deleted,
            (None, true) => ChangeType::Moved,
            (None, false) => ChangeType::Modified,
        }
    }

    /// Sets a backend-only change type.
    ///
    /// Panics if paths can already determine the type.
    pub fn set_change_type(mut self, changed_type: ChangeType) -> Self {
        assert!(
            changed_type.needs_a_backend(),
            "{changed_type:?} is readable from the paths; do not store it"
        );
        self.changed_type = Some(changed_type);
        self
    }

    /// The same file, with what it gained and lost.
    pub fn set_stats(mut self, stats: Stats) -> Self {
        self.stats = Some(stats);
        self
    }

    /// What this file gained and lost, or `None` when nothing counted it.
    pub fn get_stats(&self) -> Option<Stats> {
        self.stats
    }

    /// Whether this is an unresolved merge.
    pub fn is_conflicted(&self) -> bool {
        self.get_change_type() == ChangeType::Conflicted
    }

    /// Whether this file moved.
    pub fn is_moved(&self) -> bool {
        self.get_change_type() == ChangeType::Moved
    }

    /// The previous path of a renamed file.
    pub fn previous_path(&self) -> Option<&RepoPath> {
        match (&self.original, &self.modified) {
            (Some(before), Some(after)) if before != after => Some(before),
            _ => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::Path;

    fn a_comparison() -> Revs {
        Revs::worktree_against(crate::Oid::new("b87b24c"))
    }

    #[test]
    fn the_ordinary_cases_come_from_the_paths() {
        assert_eq!(
            File::added(at("new.rs"), a_comparison()).get_change_type(),
            ChangeType::Added
        );
        assert_eq!(
            File::renamed(at("o.rs"), at("n.rs"), a_comparison()).get_change_type(),
            ChangeType::Moved
        );
    }

    #[test]
    fn the_backend_supplies_only_what_the_paths_cannot_say() {
        let untracked =
            File::added(at("new.rs"), a_comparison()).set_change_type(ChangeType::Untracked);
        assert_eq!(untracked.get_change_type(), ChangeType::Untracked);
        assert_eq!(
            untracked.change_type_of_paths(),
            ChangeType::Added,
            "the paths still say what they say"
        );
    }

    #[test]
    #[should_panic(expected = "readable from the paths")]
    fn storing_a_derivable_change_is_refused() {
        File::added(at("new.rs"), a_comparison()).set_change_type(ChangeType::Added);
    }

    fn at(relative: &str) -> RepoPath {
        RepoPath::new(relative, Path::new("/repo"))
    }

    fn revs() -> Revs {
        Revs::worktree_against(crate::Oid::new("b87b24c"))
    }

    #[test]
    fn an_added_file_exists_only_on_the_modified_side() {
        let file = File::added(at("new.rs"), revs());
        assert_eq!(file.is_one_sided(), Some(DiffVersion::Modified));
        assert_eq!(file.path_of_version(DiffVersion::Original), None);
        assert_eq!(file.path().as_str(), "new.rs");
    }

    #[test]
    fn a_deleted_file_exists_only_on_the_original_side() {
        let file = File::deleted(at("gone.rs"), revs());
        assert_eq!(file.is_one_sided(), Some(DiffVersion::Original));
        assert_eq!(file.path_of_version(DiffVersion::Modified), None);
        assert_eq!(file.path().as_str(), "gone.rs", "still has a name");
    }

    #[test]
    fn a_rename_is_read_from_the_paths_rather_than_stored() {
        let file = File::renamed(at("old.rs"), at("new.rs"), revs());
        assert!(file.is_renamed());
        assert_eq!(file.path().as_str(), "new.rs");
        assert_eq!(file.previous_path().map(RepoPath::as_str), Some("old.rs"));
        assert_eq!(file.is_one_sided(), None, "both sides exist");
    }

    #[test]
    fn a_file_at_one_path_of_version_both_sides_is_not_a_rename() {
        let file = File::unchanged_path(at("src/main.rs"), revs());
        assert!(!file.is_renamed());
        assert_eq!(file.previous_path(), None);
    }

    #[test]
    fn a_one_sided_file_is_not_a_rename() {
        assert!(!File::added(at("new.rs"), revs()).is_renamed());
        assert!(!File::deleted(at("gone.rs"), revs()).is_renamed());
        assert_eq!(File::deleted(at("gone.rs"), revs()).previous_path(), None);
    }

    #[test]
    fn the_paths_say_what_happened() {
        assert_eq!(
            File::added(at("new.rs"), revs()).get_change_type(),
            ChangeType::Added
        );
        assert_eq!(
            File::deleted(at("gone.rs"), revs()).get_change_type(),
            ChangeType::Deleted
        );
        assert_eq!(
            File::renamed(at("old.rs"), at("new.rs"), revs()).get_change_type(),
            ChangeType::Moved
        );
        assert_eq!(
            File::unchanged_path(at("same.rs"), revs()).get_change_type(),
            ChangeType::Modified
        );
    }

    #[test]
    fn the_paths_cannot_say_the_other_two() {
        for file in [
            File::added(at("a.rs"), revs()),
            File::deleted(at("d.rs"), revs()),
            File::renamed(at("o.rs"), at("n.rs"), revs()),
            File::unchanged_path(at("m.rs"), revs()),
        ] {
            assert!(
                !file.get_change_type().needs_a_backend(),
                "{:?}",
                file.get_change_type()
            );
        }
        assert!(ChangeType::Untracked.needs_a_backend());
        assert!(ChangeType::Conflicted.needs_a_backend());
    }

    #[test]
    fn a_file_on_neither_side_cannot_be_built() {
        assert_eq!(File::new(None, None, revs()), Err(Nowhere));
        assert!(File::new(Some(at("a.rs")), None, revs()).is_ok());
    }

    #[test]
    fn a_side_is_named_the_way_git_names_it() {
        let file = File::unchanged_path(at("src/main.rs"), revs());
        assert_eq!(
            file.name(DiffVersion::Original).as_deref(),
            Some("b87b24c:src/main.rs")
        );
        assert_eq!(
            file.name(DiffVersion::Modified).as_deref(),
            Some("worktree:src/main.rs")
        );
    }

    #[test]
    fn two_versions_of_one_path_are_named_differently() {
        let staged = File::unchanged_path(
            at("src/main.rs"),
            Revs::new(Rev::Commit(crate::Oid::new("b87b24c")), Rev::Index),
        );
        let working = File::unchanged_path(at("src/main.rs"), revs());
        assert_ne!(
            staged.name(DiffVersion::Modified),
            working.name(DiffVersion::Modified)
        );
    }

    #[test]
    fn a_conflict_side_is_named_by_its_stage() {
        let file = File::unchanged_path(
            at("src/main.rs"),
            Revs::new(
                Rev::Conflict(crate::Stage::Ours),
                Rev::Conflict(crate::Stage::Theirs),
            ),
        );
        assert_eq!(
            file.name(DiffVersion::Original).as_deref(),
            Some(":2:src/main.rs")
        );
        assert_eq!(
            file.name(DiffVersion::Modified).as_deref(),
            Some(":3:src/main.rs")
        );
    }

    #[test]
    fn a_side_the_file_is_not_on_has_no_name() {
        let added = File::added(at("new.rs"), revs());
        assert_eq!(added.name(DiffVersion::Original), None);
        assert!(added.name(DiffVersion::Modified).is_some());
    }

    #[test]
    fn a_missing_side_still_says_where_it_looked() {
        let added = File::added(at("new.rs"), revs());
        assert_eq!(added.path_of_version(DiffVersion::Original), None);
        assert_eq!(
            added.rev(DiffVersion::Original),
            &Rev::Commit(crate::Oid::new("b87b24c"))
        );
    }

    #[test]
    fn a_rename_is_named_under_each_side_own_path() {
        let moved = File::renamed(at("old.py"), at("new.rs"), revs());
        assert_eq!(
            moved.name(DiffVersion::Original).as_deref(),
            Some("b87b24c:old.py")
        );
        assert_eq!(
            moved.name(DiffVersion::Modified).as_deref(),
            Some("worktree:new.rs")
        );
    }
}
