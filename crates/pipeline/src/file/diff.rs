//! What changed, and which line sits opposite which.
//!
//! Stages three and four, together because neither is worth a file of its own:
//! one calls the engine, the other hands its result to `align`. Both are pure.

use align::Alignment;
use anyhow::{Context, Result};
use vscode_diff::LinesDiff;

/// Answers stage three: what changed.
///
/// Only ever called for a file with two sides. A file that exists on one side
/// is not compared at all — there is no "before", and green-lighting every
/// line of a new file says nothing the word "added" does not. VSCode arrived
/// at the same answer and stopped opening a diff editor for added, untracked
/// and deleted files, because an empty left-hand side "did not provide much
/// value". See D23.
pub fn compute(before: &[&str], after: &[&str]) -> Result<LinesDiff> {
    let options = vscode_diff::Options::default().with_moves();
    vscode_diff::compute(before, after, &options).context("computing the diff")
}

/// Answers stage four: which line sits opposite which.
///
/// Takes the diff by value and copies the two files in, so what comes out owns
/// everything it describes and stage five can simply return it.
pub fn align(diff: LinesDiff, before: &[&str], after: &[&str]) -> Result<Alignment> {
    Alignment::try_new(diff, before, after)
        .map_err(|_| anyhow::anyhow!("the diff does not describe these two files"))
}
