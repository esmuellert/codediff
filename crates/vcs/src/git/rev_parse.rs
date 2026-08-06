//! `git rev-parse` — finding the repository, and resolving revisions.

use std::path::{Path, PathBuf};

use super::run;
use crate::error::{Error, Result};
use crate::repo::Repo;

/// Finds the repository containing `path`.
///
/// Works from any subdirectory, which is why it asks git rather than walking
/// upwards looking for `.git`: a linked worktree has a `.git` *file*, a
/// submodule's git dir lives in the parent's `.git/modules`, and
/// `GIT_DIR`/`GIT_WORK_TREE` override both.
pub fn discover(path: &Path) -> Result<Repo> {
    let start = if path.is_dir() {
        path.to_path_buf()
    } else {
        path.parent().unwrap_or(Path::new(".")).to_path_buf()
    };
    if !start.exists() {
        return Err(Error::NoRepository {
            path: path.to_path_buf(),
        });
    }

    let root = run::run_line(&start, &["rev-parse", "--show-toplevel"]).map_err(|e| match e {
        // git's own message here is long and mentions "not a git repository";
        // ours says which path we were asked about.
        Error::Git { .. } => Error::NoRepository {
            path: path.to_path_buf(),
        },
        other => other,
    })?;
    let git_dir = run::run_line(&start, &["rev-parse", "--absolute-git-dir"])?;

    Ok(Repo {
        root: PathBuf::from(root),
        control_dir: PathBuf::from(git_dir),
    })
}

/// Resolves a revision to a full object id.
/// The tree every git repository has, holding nothing.
///
/// A repository with no commit yet still has a "before" side: it is empty.
/// Git itself carries this id in every version, so nothing has to be created
/// for it to be diffed against — `git diff $EMPTY_TREE` works in a repository
/// whose first commit has not been made.
pub const EMPTY_TREE: &str = "4b825dc642cb6eb9a060e54bf8d69288fbee4904";

/// Resolves `rev`, answering with the empty tree when `HEAD` is unborn.
///
/// A repository that has been `git init`-ed and had files added has no `HEAD`
/// to resolve, and everything in it is a change. Failing there refused to
/// review the one moment when a reviewer has the most to look at.
pub fn resolve_or_empty(repo: &Repo, rev: &str) -> Result<super::Oid> {
    match resolve(repo, rev) {
        Err(Error::UnknownRevision { .. }) if rev == "HEAD" && unborn(repo) => {
            Ok(super::Oid::new(EMPTY_TREE))
        }
        other => other,
    }
}

/// Whether this repository has no commit at all.
///
/// Asked only after `HEAD` fails to resolve, and asked of `HEAD` itself rather
/// than of the branch: a detached `HEAD` pointing at nothing is not the same
/// as a name that is merely misspelled, and only this tells them apart.
fn unborn(repo: &Repo) -> bool {
    run::run_line(&repo.root, &["symbolic-ref", "--quiet", "HEAD"]).is_ok_and(|r| !r.is_empty())
}

pub fn resolve(repo: &Repo, rev: &str) -> Result<super::Oid> {
    // `--verify` makes git fail on an ambiguous or unknown name instead of
    // echoing it back, and `^{commit}` peels a tag to what it points at.
    let text = run::run_line(&repo.root, &["rev-parse", "--verify", "--quiet", rev]).map_err(
        |e| match e {
            Error::Git { .. } => Error::UnknownRevision {
                rev: rev.to_owned(),
            },
            other => other,
        },
    )?;
    if text.is_empty() {
        return Err(Error::UnknownRevision {
            rev: rev.to_owned(),
        });
    }
    Ok(super::Oid::new(text))
}
