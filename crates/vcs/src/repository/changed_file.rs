//! Git's records, in the reviewer's terms.
//!
//! The seam, and the one file that names both vocabularies. Everything in
//! `git` speaks git — `XY` codes, status letters, similarity scores — and
//! everything above `vcs` speaks [`ChangedFile`]. This is where one becomes
//! the other.
//!
//! **This is the file a second backend forks**, not [`Repository`]. A
//! mercurial backend would parse its own records into its own words and need
//! its own translation to these; the four operations above it would not
//! change. See D67.
//!
//! [`Repository`]: super::Repository

use file_types::{ChangeType, ChangedFile, File, RepoPath, Revs};

use crate::git::diff::name_status::Change;
use crate::git::status::{Code, Entry};

/// Git's model, in the reviewer's terms.
///
/// The one place the two vocabularies meet. Git reports two codes because it
/// compares three things — `HEAD`, the index and the working tree — while a
/// reviewer looking at "what changed since the last commit" wants one answer.
/// The index code wins where they differ, since it is the one that describes
/// the file's relationship to `HEAD`.
pub fn to_file_diff(entry: Entry, root: &std::path::Path, revs: Revs) -> ChangedFile {
    let change = match (entry.xy.index, entry.xy.worktree) {
        // Unresolved merges first: nothing else about the codes matters.
        (Code::Unmerged, _) | (_, Code::Unmerged) => ChangeType::Conflicted,
        (_, Code::Untracked) => ChangeType::Untracked,
        (_, Code::Ignored) => ChangeType::Untracked,
        (Code::Renamed | Code::Copied, _) => ChangeType::Moved,
        (Code::Added, _) => ChangeType::Added,
        // Deleted in the index but present on disk is a file staged for
        // deletion and then rewritten — the content differs from HEAD, so it
        // reads as a modification.
        (Code::Deleted, Code::Unmodified) => ChangeType::Deleted,
        (_, Code::Deleted) => ChangeType::Deleted,
        _ => ChangeType::Modified,
    };

    // The one place a path gains its absolute form, because this is the first
    // place that has both git's spelling and the root. Which versions exist is
    // recorded here, in the paths themselves — `File`'s pair *is* that fact,
    // and `ChangedFile` stores nothing that could contradict it.
    let path = RepoPath::new(entry.path, root);
    let file = match (change, entry.original) {
        (ChangeType::Added | ChangeType::Untracked, _) => File::added(path, revs),
        (ChangeType::Deleted, _) => File::deleted(path, revs),
        (_, Some(previous)) => File::renamed(RepoPath::new(previous, root), path, revs),
        (_, None) => File::unchanged_path(path, revs),
    };

    // Only the two the paths cannot express are carried; the rest is read back
    // off `file`, so `Added` and `Moved` have exactly one source.
    if change.needs_a_backend() {
        ChangedFile::reported(file, change)
    } else {
        ChangedFile::new(file)
    }
}

/// One record, in the reviewer's terms.
///
/// The counterpart of [`to_file_diff`], which does
/// the same for a status record. Both live in this crate because it is the
/// only one allowed to know both vocabularies.
pub fn to_changed_file(change: Change, root: &std::path::Path, revs: Revs) -> ChangedFile {
    let path = RepoPath::new(change.path, root);
    let file = match (change.letter, change.original) {
        ('A', _) => File::added(path, revs),
        ('D', _) => File::deleted(path, revs),
        // A copy is a move as far as a reviewer is concerned: the question
        // either asks is "what does the new content say", and the old path is
        // shown beside it either way.
        ('R' | 'C', Some(from)) => File::renamed(RepoPath::new(from, root), path, revs),
        _ => File::unchanged_path(path, revs),
    };
    // `U` is an unresolved merge, which the paths cannot say. Nothing else
    // here needs a backend: added, deleted and moved are all readable from
    // the pair of paths.
    if change.letter == 'U' {
        return ChangedFile::reported(file, ChangeType::Conflicted);
    }
    ChangedFile::new(file)
}
