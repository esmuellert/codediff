//! Asking the repository, and attaching what it counted.
//!
//! Every file the request found, flat and carrying its line counts. How they
//! are grouped is not decided here: a file already knows which two revisions
//! it compares, so grouping is reading a field, and the layer that draws the
//! headings is the one that phrases them.

use anyhow::{Context, Result};
use file_types::File;
use vcs::Repository;

use crate::list::Request;

/// Every file the request finds, each with what it gained and lost.
///
/// A path that is staged and then edited again yields two, which is the honest
/// answer: they are two comparisons of it, carrying different revisions.
pub fn read(request: &Request) -> Result<Vec<File>> {
    let mut repository = Repository::open(&request.repo).context("opening a repository")?;
    let changes = repository
        .changes(&request.diff_type, &request.pathspec)
        .context("listing changed files")?;

    // A failure to count is not a failure to review: the list is still correct
    // without the numbers, so a repository that will not answer loses the
    // counts rather than the whole screen.
    let counts = repository
        .counts(&request.diff_type, &request.pathspec)
        .unwrap_or_default();

    Ok(changes
        .into_iter()
        .flat_map(|group| group.files)
        .map(|file| match counts.get(file.path().as_str()) {
            Some(&stats) => file.set_stats(stats),
            None => file,
        })
        .collect())
}
