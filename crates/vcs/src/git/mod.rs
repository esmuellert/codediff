//! Git command runners and parsers.
//!
//! Each module handles one Git command. The repository layer converts parsed
//! records into the shared `file-types` vocabulary.

pub mod cat_file;
pub mod diff;
pub mod merge_base;
pub mod rev_parse;
pub mod run;
pub mod stage;
pub mod status;
pub mod worktree;

use file_types::{DiffVersion, File, FileContent, Rev, Revs};

use crate::Repo;
use crate::error::Result;
use crate::repository::DiffType;
use status::{Entry, Untracked};

/// Which git command answers a comparison.
pub enum GitCommand {
    /// `git status` — yields two comparisons (index vs HEAD, worktree vs index).
    Worktree,
    /// `git diff <args>` — one comparison.
    Diff { args: Vec<String>, revs: Revs },
}

/// Resolves revision names to ids and picks the command shape.
pub fn resolve_command(repo: &Repo, diff_type: &DiffType) -> Result<GitCommand> {
    let commit = |name: &str| -> Result<Rev> { Ok(Rev::Commit(rev_parse::resolve(repo, name)?)) };

    Ok(match diff_type {
        DiffType::Worktree => GitCommand::Worktree,
        DiffType::Against(rev) => GitCommand::Diff {
            args: vec![rev.clone()],
            revs: Revs::new(commit(rev)?, Rev::Worktree),
        },
        DiffType::Between(a, b) => GitCommand::Diff {
            args: vec![a.clone(), b.clone()],
            revs: Revs::new(commit(a)?, commit(b)?),
        },
        DiffType::MergeBase(base, target) => {
            // `a...b` compares the target with the merge base.
            let base = merge_base::run(repo, base, target)?;
            GitCommand::Diff {
                args: vec![base.as_str().to_owned(), target.clone()],
                revs: Revs::new(Rev::Commit(base), commit(target)?),
            }
        }
        DiffType::Staged(rev) => GitCommand::Diff {
            args: vec!["--cached".to_owned(), rev.clone()],
            revs: Revs::new(commit(rev)?, Rev::Index),
        },
    })
}

/// Lists changed paths using Git's porcelain-v2 status format.
pub fn status_entries(
    repo: &Repo,
    untracked: Untracked,
    pathspec: &[String],
) -> Result<Vec<Entry>> {
    let mut args = vec![
        "status",
        "--porcelain=v2",
        "-z",
        untracked.flag(),
        // Keep rename detection consistent with diff commands.
        "--find-renames",
    ];
    if !pathspec.is_empty() {
        args.push("--");
        args.extend(pathspec.iter().map(String::as_str));
    }
    status::parse(&run::run(&repo.root, &args)?)
}

/// Reads one file side from disk or Git, applying checkout filters when needed.
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
            // When comparing against the worktree, apply checkout filters
            // (CRLF, smudge) so the stored side matches what's on disk.
            if file.rev(version.other()) == &Rev::Worktree {
                return Ok(FileContent::from_bytes(cat_file::read_filtered(
                    repo, rev, &path,
                )?));
            }
            Ok(FileContent::from_bytes(blobs.read(rev, &path)?))
        }
    }
}
