//! Stages three and four: diff and align. Both are pure.

use align::Alignment;
use anyhow::{Context, Result};
use vscode_diff::LinesDiff;

/// Calls the C engine with VS Code's diff-editor defaults. Only for files with two sides.
pub fn compute(before: &[&str], after: &[&str]) -> Result<LinesDiff> {
    let options = vscode_diff::Options::default().ignoring_trim_whitespace();
    vscode_diff::compute(before, after, &options).context("computing the diff")
}

/// Pairs lines up from a diff result.
pub fn align(diff: LinesDiff, before: &[&str], after: &[&str]) -> Result<Alignment> {
    Alignment::try_new(diff, before, after)
        .map_err(|_| anyhow::anyhow!("the diff does not describe these two files"))
}
