//! `git merge-base` — where two branches parted.

use file_types::CommitHash;

use crate::Repo;
use crate::error::Result;
use crate::git::run as runner;

/// Returns the merge base used for an `a...b` comparison.
pub fn run(repo: &Repo, a: &str, b: &str) -> Result<CommitHash> {
    let text = runner::run_line(&repo.root, &["merge-base", "--end-of-options", a, b])?;
    Ok(CommitHash::new(text))
}
