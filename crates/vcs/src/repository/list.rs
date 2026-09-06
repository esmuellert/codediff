//! Lists changed files and their line counts.

use std::collections::HashMap;

use file_types::{ChangeType, DiffVersion, File, RepoPath, Rev, Revs, Stage, Stats};

use crate::git::diff::name_status::Change;
use crate::git::diff::numstat::{self, Counts};
use crate::git::status::{Code, Entry, Untracked};
use crate::git::{self, GitCommand};

use super::Repository;

/// Line counts keyed by the comparison's ending revision.
///
/// Each comparison has its own map, so staged and unstaged versions stay separate.
#[derive(Debug, Clone, Default)]
pub struct LineStats {
    counts: HashMap<Rev, Counts>,
}

impl LineStats {
    fn new(comparisons: impl IntoIterator<Item = (Rev, Counts)>) -> Self {
        Self {
            counts: comparisons.into_iter().collect(),
        }
    }

    /// Returns this file's counts, or `None` when they are unavailable.
    pub fn of(&self, file: &File) -> Option<Stats> {
        self.counts
            .get(file.rev(DiffVersion::Modified))?
            .get(file.path().as_str())
            .copied()
    }
}

impl Repository {
    /// Returns each changed file with its comparison revisions.
    ///
    /// A path can occur twice when it is staged and edited again.
    pub fn get_changed_files(
        &mut self,
        diff_type: &super::DiffType,
        pathspec: &[String],
    ) -> crate::Result<Vec<File>> {
        match git::resolve_command(&self.repo, diff_type)? {
            GitCommand::Worktree => {
                let entries = git::status_entries(&self.repo, Untracked::All, pathspec)?;
                let commit = self.revs()?.before;
                Ok(from_status(entries, &self.repo.root, commit))
            }
            GitCommand::Diff { args, revs } => {
                let args: Vec<&str> = args.iter().map(String::as_str).collect();
                let files = git::diff::name_status::run(&self.repo, &args, pathspec)?
                    .into_iter()
                    .map(|change| from_diff_entry(change, &self.repo.root, revs.clone()))
                    .collect();
                Ok(files)
            }
        }
    }

    /// Returns line counts for each comparison; uncountable files are omitted.
    pub fn get_line_stats(
        &mut self,
        diff_type: &super::DiffType,
        pathspec: &[String],
    ) -> crate::Result<LineStats> {
        match git::resolve_command(&self.repo, diff_type)? {
            GitCommand::Worktree => Ok(LineStats::new([
                (Rev::Worktree, numstat::unstaged(&self.repo)?),
                (Rev::Index, numstat::staged(&self.repo)?),
            ])),
            GitCommand::Diff { args, revs } => {
                let args: Vec<&str> = args.iter().map(String::as_str).collect();
                let counts = numstat::diff(&self.repo, &args, pathspec)?;
                Ok(LineStats::new([(revs.after, counts)]))
            }
        }
    }
}

// --- Turning git's output into files ---

/// Converts status entries to files, with unstaged entries first.
fn from_status(entries: Vec<Entry>, root: &std::path::Path, commit: Rev) -> Vec<File> {
    let (mut unstaged, mut staged) = (Vec::new(), Vec::new());
    for entry in entries {
        if entry.xy.worktree != Code::Unmodified || is_conflicted(&entry) {
            unstaged.push(status_entry_to_file(
                unstaged_view(&entry),
                root,
                Revs::new(unstaged_before(&entry), Rev::Worktree),
            ));
        }
        if is_staged(&entry) {
            staged.push(status_entry_to_file(
                entry,
                root,
                Revs::new(commit.clone(), Rev::Index),
            ));
        }
    }
    unstaged.append(&mut staged);
    unstaged
}

/// One parsed status line → one `File`.
pub(crate) fn status_entry_to_file(entry: Entry, root: &std::path::Path, revs: Revs) -> File {
    let change = match (entry.xy.index, entry.xy.worktree) {
        (Code::Unmerged, _) | (_, Code::Unmerged) => ChangeType::Conflicted,
        (_, Code::Untracked) => ChangeType::Untracked,
        (_, Code::Ignored) => ChangeType::Untracked,
        (Code::Renamed | Code::Copied, _) => ChangeType::Moved,
        (Code::Added, _) => ChangeType::Added,
        (Code::Deleted, Code::Unmodified) => ChangeType::Deleted,
        (_, Code::Deleted) => ChangeType::Deleted,
        _ => ChangeType::Modified,
    };

    let path = RepoPath::new(entry.path, root);
    let file = match (change, entry.original) {
        (ChangeType::Added | ChangeType::Untracked, _) => File::added(path, revs),
        (ChangeType::Deleted, _) => File::deleted(path, revs),
        (_, Some(previous)) => File::renamed(RepoPath::new(previous, root), path, revs),
        (_, None) => File::unchanged_path(path, revs),
    };

    if change.needs_a_backend() {
        file.set_change_type(change)
    } else {
        file
    }
}

/// One parsed `git diff --name-status` line → one `File`.
fn from_diff_entry(change: Change, root: &std::path::Path, revs: Revs) -> File {
    let path = RepoPath::new(change.path, root);
    let file = match (change.letter, change.original) {
        ('A', _) => File::added(path, revs),
        ('D', _) => File::deleted(path, revs),
        ('R' | 'C', Some(from)) => File::renamed(RepoPath::new(from, root), path, revs),
        _ => File::unchanged_path(path, revs),
    };
    if change.letter == 'U' {
        return file.set_change_type(ChangeType::Conflicted);
    }
    file
}

/// Returns the before revision for an unstaged entry.
///
/// Conflicted paths use stage 2; other paths use the index.
fn unstaged_before(entry: &Entry) -> Rev {
    if is_conflicted(entry) {
        return Rev::Conflict(Stage::Ours);
    }
    Rev::Index
}

fn is_conflicted(entry: &Entry) -> bool {
    entry.xy.index == Code::Unmerged || entry.xy.worktree == Code::Unmerged
}

fn is_staged(entry: &Entry) -> bool {
    entry.xy.index != Code::Unmodified
        && entry.xy.index != Code::Untracked
        && entry.xy.index != Code::Ignored
        && !is_conflicted(entry)
}

/// The entry as the unstaged comparison sees it.
fn unstaged_view(entry: &Entry) -> Entry {
    let mut copy = entry.clone();
    if copy.xy.worktree != Code::Untracked && copy.xy.worktree != Code::Ignored {
        copy.xy.index = Code::Unmodified;
        copy.original = None;
    }
    copy
}
