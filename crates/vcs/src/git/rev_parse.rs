//! `git rev-parse` — finding the repository, and resolving revisions.

use std::path::{Path, PathBuf};

use super::run;
use crate::change::Repo;
use crate::error::{Error, Result};

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
