//! `git rev-parse` — finding the repository, and resolving revisions.

use std::path::{Path, PathBuf};

use super::run;
use crate::error::{Error, Result};
use crate::repo::Repo;

/// Finds the repository containing `path` using `git rev-parse`.
pub fn find_repo(path: &Path) -> Result<Repo> {
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
/// Git's empty tree object, used as the base of an unborn repository.
pub const EMPTY_TREE: &str = "4b825dc642cb6eb9a060e54bf8d69288fbee4904";

/// Resolves `rev`, using the empty tree for an unborn `HEAD`.
pub fn resolve_or_empty(repo: &Repo, rev: &str) -> Result<file_types::Oid> {
    match resolve(repo, rev) {
        Err(Error::UnknownRevision { .. }) if rev == "HEAD" && unborn(repo) => {
            Ok(file_types::Oid::new(EMPTY_TREE))
        }
        other => other,
    }
}

/// Whether `HEAD` is an unborn symbolic reference.
fn unborn(repo: &Repo) -> bool {
    run::run_line(&repo.root, &["symbolic-ref", "--quiet", "HEAD"]).is_ok_and(|r| !r.is_empty())
}

pub fn resolve(repo: &Repo, rev: &str) -> Result<file_types::Oid> {
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
    Ok(file_types::Oid::new(text))
}
