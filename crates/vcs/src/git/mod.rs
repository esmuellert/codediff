//! The git backend: one file per command.
//!
//! ```text
//! run             spawn git, capture output
//! rev_parse       --show-toplevel | --absolute-git-dir | --verify
//! status          --porcelain=v2 -z
//! diff/           --name-status -z, --numstat -z
//! merge_base      git merge-base
//! cat_file        git cat-file --batch | --filters
//! worktree        std::fs — the working tree
//! ```
//!
//! Each file runs a command and parses its output in git's vocabulary.
//! Translation to the reviewer's types happens in `repository/changed_file.rs`.

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

/// Which git command answers a comparison.
pub enum Plan {
    /// `git status` — yields two comparisons (index vs HEAD, worktree vs index).
    Worktree,
    /// `git diff <args>` — one comparison.
    Diff { args: Vec<String>, revs: Revs },
}

/// Resolves revision names to ids and picks the command shape.
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
        // Without --find-renames a moved file appears as an unrelated add
        // and delete. Forced for the same reason `diff` forces it.
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
    let Some(path) = file.path_of_version(version).cloned() else {
        return Ok(FileContent::Absent);
    };
    match file.rev(version).stored() {
        None => Ok(FileContent::from_bytes(worktree::read(&path)?)),
        Some(rev) => {
            // Against the working tree, the stored side is converted the way a
            // checkout would convert it. A repository with `core.autocrlf`
            // stores LF and checks out CRLF, so comparing the stored bytes
            // with the bytes on disk marked every line changed — measured,
            // on a file where one line had been edited. The same is true of
            // any clean/smudge filter.
            if file.rev(version.other()) == &Rev::Worktree {
                return Ok(FileContent::from_bytes(cat_file::filtered(repo, rev, &path)?));
            }
            Ok(FileContent::from_bytes(blobs.read(rev, &path)?))
        }
    }
}
