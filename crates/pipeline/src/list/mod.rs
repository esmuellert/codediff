//! One request for a set of files, to something the interface can draw.
//!
//! ---
//!
//! Admission criterion: is this a step between a request and a list of files?
//!
//! | | file | |
//! |---|---|---|
//! | | [`request`] | what set of files to show, and where from |
//! | 1 | [`entries`] | ask the repository, and attach what it counted |
//!
//! **One stage.** There used to be two, the first resolving what to compare
//! into the arguments of a git command. That belonged to the backend and is
//! there now, so what is left is asking and attaching the counts. See D67.
//!
//! **A flat list, not groups.** A file carries the two revisions it compares,
//! so which group it is in is a field on it rather than a container around it,
//! and how the groups are headed is a question for whatever draws headings.
//! The plugin this replaces kept a fixed pair of lists and had to write *"we
//! treat everything as unstaged for explorer compatibility"* the first time it
//! compared two revisions. See D57.
//!
//! ```ignore
//! let files = list::run(&list::Request::worktree(root))?;
//! ```

pub mod entries;
mod request;

pub use request::Request;

use anyhow::Result;
use file_types::File;

/// Runs the request and hands over every file it found.
pub fn run(request: &Request) -> Result<Vec<File>> {
    entries::read(request)
}
