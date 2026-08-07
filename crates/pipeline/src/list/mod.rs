//! One request for a set of files, to something the interface can draw.
//!
//! ---
//!
//! Admission criterion: is this a step between a request and a list of files?
//!
//! | | file | |
//! |---|---|---|
//! | | [`request`] | what set of files to show, and where from |
//! | 1 | [`entries`] | ask the repository, and say the answer in the explorer's words |
//!
//! **One stage.** There used to be two, the first resolving what to compare
//! into the arguments of a git command. That belonged to the backend and is
//! there now, so what is left is a translation: the repository answers in its
//! own words and the explorer needs them in its. See D67.
//!
//! **A group is a revision pair.** "Staged Changes" is the name for comparing
//! the index against a commit, not a category a file belongs to, so adding a
//! way to compare is one arm of [`DiffType`](vcs::DiffType) and nothing else.
//! The plugin this replaces kept a fixed pair of lists and had to write *"we
//! treat everything as unstaged for explorer compatibility"* the first time it
//! compared two revisions. See D57.
//!
//! ```ignore
//! let groups = list::run(&list::Request::worktree(root))?;
//! ```

pub mod entries;
mod request;

pub use request::Request;

use anyhow::Result;
use explorer::Groups;

/// Runs the request and hands over the groups.
pub fn run(request: &Request) -> Result<Groups> {
    entries::read(request)
}

/// Every file the request found, flat.
///
/// For a caller that wants files rather than a list to draw — `debug
/// diff-file` is the one. A path in two groups yields two, which is the honest
/// answer: they are two comparisons of it.
pub fn files(request: &Request) -> Result<Vec<file_types::ChangedFile>> {
    Ok(run(request)?
        .into_iter()
        .flat_map(|group| group.files)
        .map(|entry| entry.file)
        .collect())
}
