//! Splitting what git reported into the comparisons it describes.
//!
//! Git reports two codes per file because it holds three things: a commit, the
//! index, and the working tree. A reviewer opening one file wants one of those
//! comparisons, so a file reported as `MM` belongs in **two** of these lists —
//! its unstaged diff and its staged diff are different diffs, and neither is a
//! duplicate of the other.
//!
//! Each list carries the revisions of its own comparison, so a file taken from
//! one already knows which two versions to read. Nothing downstream has to be
//! told which list it came out of.

use file_types::{File, Rev, Revs};

use crate::git::status::{Code, Entry};
use crate::repository::changed_file::to_file_diff;

/// Files that are all the same comparison.
///
/// The neutral counterpart of what a file list calls a group. `vcs` names it
/// itself rather than borrowing that word, because a backend must not know
/// what a file list is.
///
/// **No heading.** What a heading says is derivable from [`revs`](Self::revs)
/// — a comparison against the index is what "Staged Changes" means — so
/// storing it here would be storing the same fact twice, in the layer least
/// able to phrase it for a reader. See D57.
#[derive(Debug, Clone)]
pub struct Changes {
    pub revs: Revs,
    pub files: Vec<File>,
}

impl Changes {
    pub fn is_empty(&self) -> bool {
        self.files.is_empty()
    }
}

/// One status, read as the two comparisons it describes.
///
/// One `git status`, read twice. Running it once matters: a file staged
/// between two calls would appear in neither list or in both.
pub fn split(entries: Vec<Entry>, root: &std::path::Path, commit: Rev) -> Vec<Changes> {
    let (mut unstaged, mut staged) = (Vec::new(), Vec::new());
    for entry in entries {
        if entry.xy.worktree != Code::Unmodified {
            unstaged.push(to_file_diff(
                unstaged_view(&entry),
                root,
                Revs::new(Rev::Index, Rev::Worktree),
            ));
        }
        if is_staged(&entry) {
            staged.push(to_file_diff(
                entry,
                root,
                Revs::new(commit.clone(), Rev::Index),
            ));
        }
    }

    vec![
        Changes {
            revs: Revs::new(Rev::Index, Rev::Worktree),
            files: unstaged,
        },
        Changes {
            revs: Revs::new(commit, Rev::Index),
            files: staged,
        },
    ]
}

/// Whether git has anything staged for this path.
///
/// Untracked files have no index entry at all, so their index code is not a
/// change to it however git spells it.
fn is_staged(entry: &Entry) -> bool {
    entry.xy.index != Code::Unmodified
        && entry.xy.index != Code::Untracked
        && entry.xy.index != Code::Ignored
}

/// The entry as the *unstaged* comparison sees it.
///
/// What is staged is irrelevant on this side: a file added to the index and
/// then edited is, against the index, a plain modification. Leaving the index
/// code in place would make it read as an addition of a file the index already
/// holds.
fn unstaged_view(entry: &Entry) -> Entry {
    let mut copy = entry.clone();
    if copy.xy.worktree != Code::Untracked && copy.xy.worktree != Code::Ignored {
        copy.xy.index = Code::Unmodified;
        // A rename is staged, never unstaged: what the working tree differs
        // from is the file already at the new path.
        copy.original = None;
    }
    copy
}
