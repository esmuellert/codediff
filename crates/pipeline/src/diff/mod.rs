//! Four stages for one file: read both sides → diff → align → hand over.
//!
//! All four run on a worker thread (see [`service`]).
//!
//! ```ignore
//! let mut files = diff::DiffWorker::start();
//! files.want(&changed);          // returns at once
//! let response = files.poll();   // next frame, or the one after
//! ```
//!
use align::Alignment;
use anyhow::{Context, Result};
use vscode_diff::LinesDiff;

pub mod contents;
pub mod runner;
pub mod worker;

pub use runner::{Diff, DiffContent, Runner, SingleFile};
pub use worker::{DiffWorker, Response};

/// Calls the C engine with strict whitespace. Only for files with two sides.
pub fn compute(before: &[&str], after: &[&str]) -> Result<LinesDiff> {
    let options = vscode_diff::Options::default();
    vscode_diff::compute(before, after, &options).context("computing the diff")
}

/// Pairs lines up from a diff result.
pub fn align(diff: LinesDiff, before: &[&str], after: &[&str]) -> Result<Alignment> {
    Alignment::try_new(diff, before, after)
        .map_err(|_| anyhow::anyhow!("the diff does not describe these two files"))
}

#[cfg(test)]
mod tests {
    use super::compute;

    #[test]
    fn production_keeps_leading_and_trailing_whitespace_changes() {
        let diff = compute(&["  value"], &["value  "]).unwrap();

        assert!(!diff.is_empty());
    }
}
