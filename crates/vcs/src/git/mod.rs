//! The git backend: one file per command, and nothing else.
//!
//! **Private to this crate.** Every file below is named as git spells the
//! command it runs, so the file tree answers "which command is this?" before
//! you open anything:
//!
//! ```text
//! run             spawn git, capture what it printed, fail loudly
//! rev_parse       git rev-parse --show-toplevel | --absolute-git-dir | --verify
//! status          git status --porcelain=v2 -z
//! diff/           git diff
//! ├ name_status     --name-status -z    which files, and what happened
//! └ numstat         --numstat -z        how many lines each gained and lost
//! merge_base      git merge-base
//! cat_file        git cat-file --batch | --filters
//! worktree        std::fs — the third thing git compares, and not a command
//! ```
//!
//! Each of them **runs a command and parses what it printed into git's own
//! words** — `XY` codes, status letters, similarity scores. None of them
//! decides anything, and none produces the reviewer's vocabulary; that is
//! [`repository`](crate::repository), one level up, and
//! [`changed_file`](crate::repository::changed_file) is the seam.
//!
//! Everything here is a free function over a [`Repo`]. What a session
//! accumulates — what has been resolved, which child is open — belongs to
//! [`Repository`](crate::Repository).
//!
//! This file is the door: it turns a [`DiffType`] into the command that
//! answers it, which is the one piece of git knowledge that is not itself a
//! command. See D67.

pub mod cat_file;
pub mod diff;
pub mod merge_base;
pub mod rev_parse;
pub mod run;
pub mod status;
pub mod worktree;

use file_types::{DiffVersion, File, FileContent, Rev, Revs};

use crate::Repo;
use crate::error::Result;
use crate::repository::DiffType;
use status::{Entry, Untracked};

/// Which command answers a comparison, and what its answer will mean.
///
/// Two shapes, because git has two: the working tree is described by a status,
/// and everything else by a diff. A status describes three things at once and
/// so yields two comparisons; a diff describes two things and yields one.
pub enum Plan {
    /// `git status`, which the caller reads as two comparisons.
    Worktree,
    /// `git diff <args>`, one comparison.
    Diff {
        /// What goes after `diff`.
        args: Vec<String>,
        /// What those arguments mean in the reviewer's terms.
        revs: Revs,
    },
}

/// Resolves every revision the comparison names, and picks the command.
///
/// Names become ids here rather than staying as typed, because a name moves:
/// `main` an hour into a review is not the `main` the review opened against,
/// and an id is what says which bytes were read.
pub fn plan(repo: &Repo, diff_type: &DiffType) -> Result<Plan> {
    let commit = |name: &str| -> Result<Rev> { Ok(Rev::Commit(rev_parse::resolve(repo, name)?)) };

    Ok(match diff_type {
        DiffType::Worktree => Plan::Worktree,
        DiffType::Against(rev) => Plan::Diff {
            args: vec![rev.clone()],
            revs: Revs::new(commit(rev)?, Rev::Worktree),
        },
        DiffType::Between(a, b) => Plan::Diff {
            args: vec![a.clone(), b.clone()],
            revs: Revs::new(commit(a)?, commit(b)?),
        },
        DiffType::MergeBase(base, target) => {
            // Where the two parted, which is what `a...b` means and the only
            // reason this is its own way of comparing rather than a spelling.
            let base = merge_base::run(repo, base, target)?;
            Plan::Diff {
                args: vec![base.as_str().to_owned(), target.clone()],
                revs: Revs::new(Rev::Commit(base), commit(target)?),
            }
        }
        DiffType::Staged(rev) => Plan::Diff {
            args: vec!["--cached".to_owned(), rev.clone()],
            revs: Revs::new(commit(rev)?, Rev::Index),
        },
    })
}

/// The raw records, in git's own terms.
///
/// Runs `git --no-optional-locks status --porcelain=v2 -z`.
pub fn entries(repo: &Repo, untracked: Untracked, pathspec: &[String]) -> Result<Vec<Entry>> {
    let mut args = vec![
        "status",
        "--porcelain=v2",
        "-z",
        untracked.flag(),
        // Renames are the whole point of the `2` record; without this a moved
        // file appears as an unrelated add and delete. Forced for the same
        // reason `diff` forces it — the two are read together. See D56.
        "--find-renames",
    ];
    if !pathspec.is_empty() {
        args.push("--");
        args.extend(pathspec.iter().map(String::as_str));
    }
    status::parse(&run::run(&repo.root, &args)?)
}

/// One side of a file, from wherever that side lives.
///
/// Three places a version can be: on disk, in the object store, or in the
/// object store but wanted as a checkout would write it. Which one is decided
/// by the file's own revisions, not by which side was asked for.
pub fn read(
    repo: &Repo,
    blobs: &mut cat_file::Batch,
    file: &File,
    version: DiffVersion,
) -> Result<FileContent> {
    let Some(path) = file.on(version).cloned() else {
        return Ok(FileContent::Absent);
    };
    match file.rev(version).stored() {
        None => Ok(FileContent::of(worktree::read(&path)?)),
        Some(rev) => {
            // Against the working tree, the stored side is converted the way a
            // checkout would convert it. A repository with `core.autocrlf`
            // stores LF and checks out CRLF, so comparing the stored bytes
            // with the bytes on disk marked **every line** changed — measured,
            // on a file where one line had been edited. The same is true of
            // any clean/smudge filter.
            if file.rev(version.other()) == &Rev::Worktree {
                return Ok(FileContent::of(cat_file::filtered(repo, rev, &path)?));
            }
            Ok(FileContent::of(blobs.read(rev, &path)?))
        }
    }
}
