//! Which file this is.

use crate::{DiffVersion, RepoPath};

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
    pub fn unchanged_path(path: RepoPath) -> Self {
        Self {
            original: Some(path.clone()),
            modified: Some(path),
        }
    }

    /// A file that moved, or whose content changed under a new name.
    pub fn renamed(original: RepoPath, modified: RepoPath) -> Self {
        Self {
            original: Some(original),
            modified: Some(modified),
        }
    }

    /// A file that does not exist on the original side.
    pub fn added(path: RepoPath) -> Self {
        Self {
            original: None,
            modified: Some(path),
        }
    }

    /// A file that does not exist on the modified side.
    pub fn deleted(path: RepoPath) -> Self {
        Self {
            original: Some(path),
            modified: None,
        }
    }

    /// From a pair, refusing the one combination that is not a file.
    ///
    /// The named constructors above are clearer at a call site that knows
    /// which case it has; this is for one that does not.
    pub fn new(original: Option<RepoPath>, modified: Option<RepoPath>) -> Result<Self, Nowhere> {
        if original.is_none() && modified.is_none() {
            return Err(Nowhere);
        }
        Ok(Self { original, modified })
    }

    /// Where the file is on one side, or `None` if it is not there.
    pub fn on(&self, version: DiffVersion) -> Option<&RepoPath> {
        match version {
            DiffVersion::Original => self.original.as_ref(),
            DiffVersion::Modified => self.modified.as_ref(),
        }
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

    #[test]
    fn an_added_file_exists_only_on_the_modified_side() {
        let file = File::added(at("new.rs"));
        assert_eq!(file.only(), Some(DiffVersion::Modified));
        assert_eq!(file.on(DiffVersion::Original), None);
        assert_eq!(file.path().as_str(), "new.rs");
    }

    #[test]
    fn a_deleted_file_exists_only_on_the_original_side() {
        let file = File::deleted(at("gone.rs"));
        assert_eq!(file.only(), Some(DiffVersion::Original));
        assert_eq!(file.on(DiffVersion::Modified), None);
        assert_eq!(file.path().as_str(), "gone.rs", "still has a name");
    }

    #[test]
    fn a_rename_is_read_from_the_paths_rather_than_stored() {
        // No `kind` field to disagree with the paths. VSCode's multi-diff
        // renderer derives the same fact the same way, at paint time.
        let file = File::renamed(at("old.rs"), at("new.rs"));
        assert!(file.is_renamed());
        assert_eq!(file.path().as_str(), "new.rs");
        assert_eq!(file.previous_path().map(RepoPath::as_str), Some("old.rs"));
        assert_eq!(file.only(), None, "both sides exist");
    }

    #[test]
    fn a_file_at_one_path_on_both_sides_is_not_a_rename() {
        let file = File::unchanged_path(at("src/main.rs"));
        assert!(!file.is_renamed());
        assert_eq!(file.previous_path(), None);
    }

    #[test]
    fn a_one_sided_file_is_not_a_rename() {
        // The trap: `previous_path` reading `original` unconditionally would
        // make every deleted file look renamed from itself.
        assert!(!File::added(at("new.rs")).is_renamed());
        assert!(!File::deleted(at("gone.rs")).is_renamed());
        assert_eq!(File::deleted(at("gone.rs")).previous_path(), None);
    }

    #[test]
    fn the_paths_say_what_happened() {
        assert_eq!(File::added(at("new.rs")).change(), ChangeType::Added);
        assert_eq!(File::deleted(at("gone.rs")).change(), ChangeType::Deleted);
        assert_eq!(
            File::renamed(at("old.rs"), at("new.rs")).change(),
            ChangeType::Moved
        );
        assert_eq!(
            File::unchanged_path(at("same.rs")).change(),
            ChangeType::Modified
        );
    }

    #[test]
    fn the_paths_cannot_say_the_other_two() {
        // An untracked file's paths are indistinguishable from an added one's,
        // which is why a backend has to supply the distinction and why these
        // four can be derived rather than stored.
        for file in [
            File::added(at("a.rs")),
            File::deleted(at("d.rs")),
            File::renamed(at("o.rs"), at("n.rs")),
            File::unchanged_path(at("m.rs")),
        ] {
            assert!(!file.change().needs_a_backend(), "{:?}", file.change());
        }
        assert!(ChangeType::Untracked.needs_a_backend());
        assert!(ChangeType::Conflicted.needs_a_backend());
    }

    #[test]
    fn a_file_on_neither_side_cannot_be_built() {
        assert_eq!(File::new(None, None), Err(Nowhere));
        assert!(File::new(Some(at("a.rs")), None).is_ok());
    }
}
