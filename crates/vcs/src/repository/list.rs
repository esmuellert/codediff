//! What changed: the list of files, and how many lines each gained.

use file_types::{ChangeType, File, RepoPath, Rev, Revs, Stage};

use crate::git::diff::name_status::Change;
use crate::git::diff::numstat::{self, Counts};
use crate::git::status::{Code, Entry, Untracked};
use crate::git::{self, GitCommand};

use super::Repository;

impl Repository {
    /// Every file that differs, each carrying the two revisions it compares.
    ///
    /// A path that is staged and then edited again appears twice — one per
    /// comparison, each carrying its own revision pair.
    ///
    /// A flat list. What a repository owes its caller is the files; how
    /// they are grouped, ordered or drawn is not its question.
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

    /// How many lines each file gained and lost, by path.
    ///
    /// A failure to count is not a failure to review — the list is correct
    /// without the numbers — so this is the caller's to ignore.
    pub fn get_line_stats(
        &mut self,
        diff_type: &super::DiffType,
        pathspec: &[String],
    ) -> crate::Result<Counts> {
        match git::resolve_command(&self.repo, diff_type)? {
            GitCommand::Worktree => {
                let mut counts = numstat::unstaged(&self.repo)?;
                counts.extend(numstat::staged(&self.repo)?);
                Ok(counts)
            }
            GitCommand::Diff { args, .. } => {
                let args: Vec<&str> = args.iter().map(String::as_str).collect();
                numstat::diff(&self.repo, &args, pathspec)
            }
        }
    }
}

// --- Turning git's output into files ---

/// Every comparison a `git status` describes, as files.
///
/// Unstaged first, because that is the order a reader reviews in and nothing
/// downstream knows it.
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

/// What the working tree is compared against on the unstaged side.
///
/// The index, except for an unresolved merge — which has no index. Git holds
/// three versions of a conflicted path, at stages 1, 2 and 3, and none at
/// stage 0. Stage 2 is what the reader is merging *into*.
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
