//! `git merge-base` — where two branches parted.

use file_types::Oid;

use crate::Repo;
use crate::error::Result;
use crate::git::run as runner;

/// The commit two revisions last had in common.
///
/// What `a...b` means, and the only reason comparing against a merge base is
/// its own way of comparing rather than a spelling of comparing two revisions.
pub fn run(repo: &Repo, a: &str, b: &str) -> Result<Oid> {
    let text = runner::run_line(&repo.root, &["merge-base", a, b])?;
    Ok(Oid::new(text))
}
