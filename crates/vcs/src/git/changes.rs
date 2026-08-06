//! Splitting a status into the comparisons it describes.
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

use file_types::{ChangedFile, Rev, Revs};

use crate::error::Result;
use crate::git::status::{Code, Entry};
use crate::git::{Git, run, to_file_diff};

/// Files that are all the same comparison, in git's terms.
///
/// The neutral counterpart of what an explorer calls a group. `vcs` names it
/// itself rather than borrowing that word, because a backend must not know
/// what an explorer is — `cargo xtask lint-arch` forbids the edge, and the
/// binary is where the two vocabularies meet.
#[derive(Debug, Clone)]
pub struct Changes {
    /// What a heading would say. Git's own words, since git is what decided
    /// this group exists.
    pub name: &'static str,
    pub revs: Revs,
    pub files: Vec<ChangedFile>,
}

impl Changes {
    pub fn is_empty(&self) -> bool {
        self.files.is_empty()
    }
}

impl Git {
    /// What is not committed: the working tree against the index, and the
    /// index against the commit.
    ///
    /// One `git status`, read two ways. Running it once matters: a file staged
    /// between two calls would appear in neither list or in both.
    pub fn worktree_changes(&mut self, pathspec: &[String]) -> Result<Vec<Changes>> {
        let root = self.repo().root.clone();
        let commit = self.revs()?.before;
        let entries = self.entries(pathspec)?;

        let (mut unstaged, mut staged) = (Vec::new(), Vec::new());
        for entry in entries {
            if entry.xy.worktree != Code::Unmodified {
                unstaged.push(to_file_diff(
                    unstaged_view(&entry),
                    &root,
                    Revs::new(Rev::Index, Rev::Worktree),
                ));
            }
            if is_staged(&entry) {
                staged.push(to_file_diff(
                    entry,
                    &root,
                    Revs::new(commit.clone(), Rev::Index),
                ));
            }
        }

        Ok(vec![
            Changes {
                name: "Changes",
                revs: Revs::new(Rev::Index, Rev::Worktree),
                files: unstaged,
            },
            Changes {
                name: "Staged Changes",
                revs: Revs::new(commit, Rev::Index),
                files: staged,
            },
        ])
    }
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

impl Git {
    /// What differs between two things git can name, as one comparison.
    ///
    /// The counterpart of [`worktree_changes`](Git::worktree_changes) for
    /// every other way of comparing: one group, because two things have one
    /// difference between them. `args` is what goes after `diff`, and `revs`
    /// is what those arguments mean in the reviewer's terms — the caller
    /// supplies both because it is the caller that chose them.
    pub fn diff_changes(
        &self,
        name: &'static str,
        args: &[&str],
        revs: Revs,
        pathspec: &[String],
    ) -> Result<Vec<Changes>> {
        let root = self.repo().root.clone();
        let files = self
            .name_status(args, pathspec)?
            .into_iter()
            .map(|change| crate::git::to_changed_file(change, &root, revs.clone()))
            .collect();
        Ok(vec![Changes { name, revs, files }])
    }

    /// Where two branches parted. Runs `git merge-base`.
    pub fn merge_base(&self, a: &str, b: &str) -> Result<file_types::Oid> {
        let text = run::run_line(&self.repo().root, &["merge-base", a, b])?;
        Ok(file_types::Oid::new(text))
    }
}
