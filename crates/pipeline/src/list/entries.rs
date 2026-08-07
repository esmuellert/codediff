//! Asking the repository, and saying the answer in the explorer's words.
//!
//! The only place git's vocabulary and the explorer's are both spoken.
//! `explorer` may not name `vcs` and `vcs` may not name `explorer` — `cargo
//! xtask lint-arch` forbids both — so the translation happens here.

use anyhow::{Context, Result};
use explorer::{Entry, Group, Groups};
use vcs::{Changes, Counts, Repository};

use crate::list::Request;

/// Every file the request finds, grouped by the comparison it belongs to.
pub fn read(request: &Request) -> Result<Groups> {
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
        .filter(|group| !group.is_empty())
        .map(|group| translate(group, &counts))
        .collect())
}

/// One of the repository's groups, in the explorer's words.
///
/// The whole translation: a name, a revision pair, and files carrying their
/// line counts. Nothing is decided here — which groups exist was decided by
/// the comparison, and what is in them by the repository.
fn translate(changes: Changes, counts: &Counts) -> Group {
    let files = changes
        .files
        .into_iter()
        .map(|file| {
            let stats = counts.get(file.path().as_str()).copied();
            let entry = Entry::new(file);
            match stats {
                Some(stats) => entry.with_stats(stats),
                None => entry,
            }
        })
        .collect();
    Group::new(changes.name, changes.revs, files)
}
