//! Staging and unstaging files.

use std::path::Path;

use super::run;
use crate::error::Result;

/// Stages a file: `git add -- <path>`.
pub fn stage(repo_root: &Path, path: &str) -> Result<()> {
    run::run(repo_root, &["add", "--", path])?;
    Ok(())
}

/// Unstages a file: `git reset HEAD -- <path>`.
pub fn unstage(repo_root: &Path, path: &str) -> Result<()> {
    run::run(repo_root, &["reset", "HEAD", "--", path])?;
    Ok(())
}
