//! One request for a set of files, to something the interface can draw.
//!
//! ---
//!
//! Admission criterion: is this a step between a request and a list of files?
//! Two of them:
//!
//! | | file | |
//! |---|---|---|
//! | 1 | [`resolver`] | what am I comparing, and therefore which git command |
//! | 2 | [`entries`] | run it, and say the answer in the explorer's words |
//!
//! Two rather than five, because counting git *commands* is not counting
//! steps: reading the line counts is part of asking git about a comparison,
//! not a stage between the question and the answer.
//!
//! **A group is a revision pair.** "Staged Changes" is the name for comparing
//! the index against a commit, not a category a file belongs to, so adding a
//! way to compare is one arm of [`ExplorerDiffType`] and nothing else. The
//! plugin this replaces kept a fixed pair of lists and had to write *"we treat
//! everything as unstaged for explorer compatibility"* the first time it
//! compared two revisions. See D57.
//!
//! [`ExplorerDiffType`]: explorer::ExplorerDiffType
//!
//! ```ignore
//! let groups = list::run(&ExplorerDiffRequest::worktree(root))?;
//! ```

pub mod entries;
pub mod resolver;

use anyhow::Result;
use explorer::{ExplorerDiffRequest, Groups};

/// Runs both stages and hands over the groups.
pub fn run(request: &ExplorerDiffRequest) -> Result<Groups> {
    let resolved = resolver::resolve(request)?;
    entries::read(resolved, request)
}

/// Every file the request found, flat.
///
/// For a caller that wants files rather than a list to draw — `debug
/// diff-file` is the one. A path in two groups yields two, which is the honest
/// answer: they are two comparisons of it.
pub fn files(request: &ExplorerDiffRequest) -> Result<Vec<file_types::ChangedFile>> {
    Ok(run(request)?
        .into_iter()
        .flat_map(|group| group.files)
        .map(|entry| entry.file)
        .collect())
}
