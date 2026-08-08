//! A repository being read, and the four things a reviewer needs from one.
//!
//! ---
//!
//! **The whole surface of this crate.** `git` is private, so nothing outside
//! can run a git command, name a status code, or hold a `--cached`. A second
//! backend is a directory beside `git` and an arm in [`open`](Repository::open)
//! — not a search for every caller that reached past this.
//!
//! Four operations, because four is what a review needs:
//!
//! ```text
//! open      find the repository containing a path
//! changes   what differs, grouped by the comparison it belongs to
//! counts    how many lines each file gained and lost
//! read      one side of one file
//! ```
//!
//! There is no trait. The contract is the *types*: everything returned is
//! `file-types`, and `cargo xtask lint-arch` forbids that crate from naming
//! this one, so no git concept can reach a reviewer. A second backend earns a
//! trait extracted from two real implementations; one guessed from a single
//! implementor was checking nothing. See D30.

pub mod changed_file;
mod changes;
mod diff_type;

pub use changes::Changes;
pub use diff_type::DiffType;

use std::path::Path;

use file_types::{ChangedFile, DiffVersion, FileContent, RepoPath, Revs};

use crate::Repo;
use crate::error::Result;
use crate::git::diff::numstat::Counts;
use crate::git::status::Untracked;
use crate::git::{self, Plan, cat_file, rev_parse};

/// An open repository.
///
/// Holds what a session accumulates — what has been resolved, which child
/// process is open — so that everything below can be a free function over a
/// [`Repo`] and none of it has to be told twice.
#[derive(Debug)]
pub struct Repository {
    repo: Repo,
    untracked: Untracked,
    /// Resolved on first use, so a list-only run never pays for the extra
    /// process — and resolved **once**, so a commit made while a review is
    /// open cannot leave half its files named against one `HEAD` and half
    /// against another.
    revs: Option<Revs>,
    /// Opened on first use, so a list-only run never pays for the child.
    blobs: Option<cat_file::Batch>,
}

impl Repository {
    /// Opens the repository containing `path`.
    ///
    /// `path` is a place to start looking, not the root: the backend discovers
    /// that, and every path built afterwards is relative to what it found.
    pub fn open(path: &Path) -> Result<Self> {
        Ok(Self {
            repo: rev_parse::discover(path)?,
            untracked: Untracked::default(),
            revs: None,
            blobs: None,
        })
    }

    /// Whether untracked files are listed at all, and how deeply.
    pub fn with_untracked(mut self, untracked: Untracked) -> Self {
        self.untracked = untracked;
        self
    }

    /// Where the repository is.
    ///
    /// The root is what paths are relative to; the control directory is what a
    /// file watcher needs to notice a branch switch.
    pub fn repo(&self) -> &Repo {
        &self.repo
    }

    /// What differs, grouped by the comparison each group is of.
    ///
    /// One group for every way of comparing except the working tree, which is
    /// two: git holds three things at once — a commit, the index, the files on
    /// disk — and a reviewer opening one file wants one pair of them.
    pub fn changes(&mut self, diff_type: &DiffType, pathspec: &[String]) -> Result<Vec<Changes>> {
        match git::plan(&self.repo, diff_type)? {
            Plan::Worktree => {
                let entries = git::entries(&self.repo, self.untracked, pathspec)?;
                let commit = self.revs()?.before;
                Ok(changes::split(entries, &self.repo.root, commit))
            }
            Plan::Diff { args, revs } => {
                let args: Vec<&str> = args.iter().map(String::as_str).collect();
                let files = git::diff::name_status::run(&self.repo, &args, pathspec)?
                    .into_iter()
                    .map(|change| {
                        changed_file::to_changed_file(change, &self.repo.root, revs.clone())
                    })
                    .collect();
                Ok(vec![Changes { revs, files }])
            }
        }
    }

    /// How many lines each file gained and lost, by path.
    ///
    /// A failure to count is not a failure to review — the list is correct
    /// without the numbers — so this is the caller's to ignore.
    pub fn counts(&mut self, diff_type: &DiffType, pathspec: &[String]) -> Result<Counts> {
        match git::plan(&self.repo, diff_type)? {
            Plan::Worktree => {
                // Two comparisons, so two runs. One map, because a path can be
                // in both — a file staged and then edited again — and keeping
                // them apart means a map per group, which is a stage of
                // plumbing for a number beside a name. See D57.
                let mut counts = git::diff::numstat::unstaged(&self.repo)?;
                counts.extend(git::diff::numstat::staged(&self.repo)?);
                Ok(counts)
            }
            Plan::Diff { args, .. } => {
                let args: Vec<&str> = args.iter().map(String::as_str).collect();
                git::diff::numstat::diff(&self.repo, &args, pathspec)
            }
        }
    }

    /// One side of one file.
    ///
    /// Takes the whole [`ChangedFile`] rather than a path so that a move reads
    /// its old path without the caller having to know that rule.
    pub fn read(&mut self, file: &ChangedFile, version: DiffVersion) -> Result<FileContent> {
        // Split so the borrow of `blobs` does not also borrow `repo`.
        if self.blobs.is_none() {
            self.blobs = Some(cat_file::Batch::open(&self.repo)?);
        }
        let blobs = self.blobs.as_mut().expect("just opened");
        git::read(&self.repo, blobs, file, version)
    }

    /// One path as it was at one revision, exactly.
    ///
    /// Not part of reviewing anything — [`read`](Self::read) is what a review
    /// uses, and it takes the file rather than a path so that a move reads its
    /// old name. This is for checking that what we read is byte for byte what
    /// the backend holds, which is S6's acceptance check and needs a way to
    /// name a version that no comparison mentions.
    ///
    /// `None` when nothing is there at that revision.
    pub fn at(&mut self, rev: &str, path: &RepoPath) -> Result<Option<Vec<u8>>> {
        if self.blobs.is_none() {
            self.blobs = Some(cat_file::Batch::open(&self.repo)?);
        }
        self.blobs.as_mut().expect("just opened").read(rev, path)
    }

    /// What the two sides of the working-tree comparison are, resolved.
    fn revs(&mut self) -> Result<Revs> {
        if self.revs.is_none() {
            let commit = rev_parse::resolve_or_empty(&self.repo, "HEAD")?;
            self.revs = Some(Revs::worktree_against(commit));
        }
        Ok(self.revs.clone().expect("just resolved"))
    }
}
