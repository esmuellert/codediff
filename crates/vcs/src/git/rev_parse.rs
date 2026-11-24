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
        // Report the path supplied by the caller.
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

/// Git's empty tree object, used as the base of an unborn repository.
pub const EMPTY_TREE: &str = "4b825dc642cb6eb9a060e54bf8d69288fbee4904";

/// Resolves `rev`, using the empty tree for an unborn `HEAD`.
pub fn resolve_or_empty(repo: &Repo, rev: &str) -> Result<file_types::CommitHash> {
    match resolve(repo, rev) {
        Err(Error::UnknownRevision { .. }) if rev == "HEAD" && unborn(repo) => {
            Ok(file_types::CommitHash::new(EMPTY_TREE))
        }
        other => other,
    }
}

/// Whether `HEAD` is an unborn symbolic reference.
fn unborn(repo: &Repo) -> bool {
    run::run_line(&repo.root, &["symbolic-ref", "--quiet", "HEAD"]).is_ok_and(|r| !r.is_empty())
}

/// Resolves a commit-ish revision to its peeled commit hash.
pub fn resolve(repo: &Repo, rev: &str) -> Result<file_types::CommitHash> {
    // `--end-of-options` stops leading `-` names from being parsed as flags,
    // and `^{commit}` peels annotated tags while rejecting blobs and trees.
    let spec = format!("{rev}^{{commit}}");
    let text = run::run_line(
        &repo.root,
        &[
            "rev-parse",
            "--verify",
            "--quiet",
            "--end-of-options",
            &spec,
        ],
    )
    .map_err(|e| match e {
        Error::Git { .. } => Error::UnknownRevision {
            rev: rev.to_owned(),
        },
        other => other,
    })?;
    if text.is_empty() {
        return Err(Error::UnknownRevision {
            rev: rev.to_owned(),
        });
    }
    Ok(file_types::CommitHash::new(text))
}
