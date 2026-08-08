//! Which file this is.

use crate::{DiffVersion, RepoPath, Rev};

/// What happened to a file between the two versions being compared.
///
/// Two groups, and the difference matters:
///
/// - `Added`, `Deleted`, `Moved` and `Modified` are **readable from the paths
///   alone**, which is what [`File::change`] does.
/// - `Untracked` and `Conflicted` are not. An untracked file looks exactly
///   like an added one, and a conflicted file looks like an ordinary
///   modification; only a version control system can tell.
///
/// So the first four are never stored anywhere — storing them would let a
/// field contradict the paths beside it — and a backend supplies only the two
/// it alone knows.
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
    /// Left unresolved by a merge. Reported so it is not silently missing;
    /// resolving one means editing the file, which this tool does not do.
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

    /// True when a version control system had to say so.
    ///
    /// The two [`File::change`] can never return.
    pub fn needs_a_backend(self) -> bool {
        matches!(self, ChangeType::Untracked | ChangeType::Conflicted)
    }
}

/// A file under review: where it is on each side of the comparison.
///
/// `None` on a side means the file does not exist there — added on the left,
/// deleted on the right. Two `Some` with different paths is a rename.
///
/// Everything a reader is told about a file is **derived** from this pair,
/// never stored beside it. That is the point: a `kind` field could disagree
/// with the paths, and a formatted `label` field already did — it fused the
/// path, the previous path and the added/deleted note into one string, after
/// which nothing could style or shorten them separately.
///
/// VSCode's `MultiDiffEditorItem` is the same pair, and its renderer likewise
/// recomputes "renamed" at paint time from `modifiedUri.path !=
/// originalUri.path` rather than storing a flag.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct File {
    original: Option<RepoPath>,
    modified: Option<RepoPath>,
    /// Which version each side was read from. Beside the paths rather than
    /// inside them, because an added file has no original path and was still
    /// looked for at a commit.
    before: Rev,
    after: Rev,
}

/// What a file's two sides were read from.
///
/// One value rather than two arguments, because every file of one review
/// shares it: a `:CodeDiff` compares the working tree against one commit, and
/// each file is another row of that same comparison. Two arguments of one type
/// would also be two arguments nothing could check the order of.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Revs {
    pub before: Rev,
    pub after: Rev,
}

impl Revs {
    pub fn new(before: Rev, after: Rev) -> Self {
        Self { before, after }
    }

    /// The comparison `:CodeDiff` makes with no arguments: a commit against
    /// the file on disk.
    ///
    /// The commit is an id rather than `HEAD`, because a name that moves
    /// cannot say which bytes were read — see [`Rev::Commit`].
    pub fn worktree_against(commit: crate::Oid) -> Self {
        Self::new(Rev::Commit(commit), Rev::Worktree)
    }

    /// What a heading calls this comparison.
    ///
    /// Derived rather than stored, because comparing against the index *is*
    /// what "Staged Changes" means. A backend that reported the name as well
    /// would be reporting the same fact twice, and the plugin this replaces
    /// did exactly that — it kept a fixed pair of lists and had to write *"we
    /// treat everything as unstaged for explorer compatibility"* the first
    /// time it compared two revisions. See D57.
    ///
    /// A reader's words, like [`Rev`]'s `Display` and unlike
    /// [`Rev::stored`](Rev::stored), which is what goes to git.
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
        }
    }

    /// A file that moved, or whose content changed under a new name.
    pub fn renamed(original: RepoPath, modified: RepoPath, revs: Revs) -> Self {
        Self {
            original: Some(original),
            modified: Some(modified),
            before: revs.before,
            after: revs.after,
        }
    }

    /// A file that does not exist on the original side.
    pub fn added(path: RepoPath, revs: Revs) -> Self {
        Self {
            original: None,
            modified: Some(path),
            before: revs.before,
            after: revs.after,
        }
    }

    /// A file that does not exist on the modified side.
    pub fn deleted(path: RepoPath, revs: Revs) -> Self {
        Self {
            original: Some(path),
            modified: None,
            before: revs.before,
            after: revs.after,
        }
    }

    /// From a pair, refusing the one combination that is not a file.
    ///
    /// The named constructors above are clearer at a call site that knows
    /// which case it has; this is for one that does not.
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
        })
    }

    /// Where the file is on one side, or `None` if it is not there.
    pub fn on(&self, version: DiffVersion) -> Option<&RepoPath> {
        match version {
            DiffVersion::Original => self.original.as_ref(),
            DiffVersion::Modified => self.modified.as_ref(),
        }
    }

    /// Which version one side was read from.
    ///
    /// Answered even for a side the file is not on, because "we looked at
    /// `HEAD` and it was not there" is what an added file is.
    pub fn rev(&self, version: DiffVersion) -> &Rev {
        match version {
            DiffVersion::Original => &self.before,
            DiffVersion::Modified => &self.after,
        }
    }

    /// How git names one side's content, if the file is on that side.
    ///
    /// Git's own spelling, except for the working tree, which git cannot name.
    /// An identity and not a path: nothing can read the file name back out of
    /// it, so whatever needs the language asks [`on`](Self::on) instead.
    pub fn name(&self, version: DiffVersion) -> Option<String> {
        let path = self.on(version)?;
        Some(match self.rev(version).stored() {
            Some(rev) => format!("{rev}:{path}"),
            None => format!("worktree:{path}"),
        })
    }

    /// The one side this file exists on, or `None` when it exists on both.
    ///
    /// What decides whether there is anything to compare. `Some` means the
    /// reader gets one column, because a second could hold nothing.
    pub fn only(&self) -> Option<DiffVersion> {
        match (&self.original, &self.modified) {
            (None, Some(_)) => Some(DiffVersion::Modified),
            (Some(_), None) => Some(DiffVersion::Original),
            _ => None,
        }
    }

    /// Which two versions this file compares.
    ///
    /// What a heading is derived from: a comparison against the index is what
    /// "Staged Changes" means, so a file already carries which group it is in.
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

    /// The path to lead with: where the file is now, or where it was if it is
    /// gone.
    ///
    /// Never `None` — a file exists on at least one side, which the
    /// constructors enforce.
    pub fn path(&self) -> &RepoPath {
        self.modified
            .as_ref()
            .or(self.original.as_ref())
            .expect("a file exists on at least one side")
    }

    /// What the paths say happened.
    ///
    /// Never `Untracked` or `Conflicted` — those are invisible in a path pair.
    /// A backend that knows better overrides this; see
    /// `vcs::ChangedFile::change`.
    pub fn change(&self) -> ChangeType {
        match (self.only(), self.is_renamed()) {
            (Some(crate::DiffVersion::Modified), _) => ChangeType::Added,
            (Some(crate::DiffVersion::Original), _) => ChangeType::Deleted,
            (None, true) => ChangeType::Moved,
            (None, false) => ChangeType::Modified,
        }
    }

    /// Where the file was, when that differs from where it is.
    ///
    /// `Some` exactly when [`is_renamed`](Self::is_renamed), so a caller that
    /// wants to show `old → new` needs one lookup rather than two.
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

    fn at(relative: &str) -> RepoPath {
        RepoPath::new(relative, Path::new("/repo"))
    }

    /// The ordinary comparison. Which revisions these are is not what any
    /// test below is about, so it is said once.
    fn revs() -> Revs {
        Revs::worktree_against(crate::Oid::new("b87b24c"))
    }

    #[test]
    fn an_added_file_exists_only_on_the_modified_side() {
        let file = File::added(at("new.rs"), revs());
        assert_eq!(file.only(), Some(DiffVersion::Modified));
        assert_eq!(file.on(DiffVersion::Original), None);
        assert_eq!(file.path().as_str(), "new.rs");
    }

    #[test]
    fn a_deleted_file_exists_only_on_the_original_side() {
        let file = File::deleted(at("gone.rs"), revs());
        assert_eq!(file.only(), Some(DiffVersion::Original));
        assert_eq!(file.on(DiffVersion::Modified), None);
        assert_eq!(file.path().as_str(), "gone.rs", "still has a name");
    }

    #[test]
    fn a_rename_is_read_from_the_paths_rather_than_stored() {
        // No `kind` field to disagree with the paths. VSCode's multi-diff
        // renderer derives the same fact the same way, at paint time.
        let file = File::renamed(at("old.rs"), at("new.rs"), revs());
        assert!(file.is_renamed());
        assert_eq!(file.path().as_str(), "new.rs");
        assert_eq!(file.previous_path().map(RepoPath::as_str), Some("old.rs"));
        assert_eq!(file.only(), None, "both sides exist");
    }

    #[test]
    fn a_file_at_one_path_on_both_sides_is_not_a_rename() {
        let file = File::unchanged_path(at("src/main.rs"), revs());
        assert!(!file.is_renamed());
        assert_eq!(file.previous_path(), None);
    }

    #[test]
    fn a_one_sided_file_is_not_a_rename() {
        // The trap: `previous_path` reading `original` unconditionally would
        // make every deleted file look renamed from itself.
        assert!(!File::added(at("new.rs"), revs()).is_renamed());
        assert!(!File::deleted(at("gone.rs"), revs()).is_renamed());
        assert_eq!(File::deleted(at("gone.rs"), revs()).previous_path(), None);
    }

    #[test]
    fn the_paths_say_what_happened() {
        assert_eq!(
            File::added(at("new.rs"), revs()).change(),
            ChangeType::Added
        );
        assert_eq!(
            File::deleted(at("gone.rs"), revs()).change(),
            ChangeType::Deleted
        );
        assert_eq!(
            File::renamed(at("old.rs"), at("new.rs"), revs()).change(),
            ChangeType::Moved
        );
        assert_eq!(
            File::unchanged_path(at("same.rs"), revs()).change(),
            ChangeType::Modified
        );
    }

    #[test]
    fn the_paths_cannot_say_the_other_two() {
        // An untracked file's paths are indistinguishable from an added one's,
        // which is why a backend has to supply the distinction and why these
        // four can be derived rather than stored.
        for file in [
            File::added(at("a.rs"), revs()),
            File::deleted(at("d.rs"), revs()),
            File::renamed(at("o.rs"), at("n.rs"), revs()),
            File::unchanged_path(at("m.rs"), revs()),
        ] {
            assert!(!file.change().needs_a_backend(), "{:?}", file.change());
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
        // These strings are what tells one version of a file from another, so
        // they have to be git's spelling rather than one of ours.
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
        // The whole point. Reviewing the staged copy and the working copy of
        // one file must not land on one name, or whatever caches by it hands
        // back the wrong answer.
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
        // "We read HEAD and the file was not there" is what an added file is,
        // and it is two facts rather than one. Folding the revision into the
        // path would lose the half that says where.
        let added = File::added(at("new.rs"), revs());
        assert_eq!(added.on(DiffVersion::Original), None);
        assert_eq!(
            added.rev(DiffVersion::Original),
            &Rev::Commit(crate::Oid::new("b87b24c"))
        );
    }

    #[test]
    fn a_rename_is_named_under_each_side_own_path() {
        // Which is also what decides the language of each side: a `.py`
        // renamed to a `.rs` is Python on the left and Rust on the right.
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
