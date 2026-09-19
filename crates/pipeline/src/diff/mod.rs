//! Four stages for one file: read both sides → diff → align → hand over.
//!
//! The public worker runs the blocking stages away from the UI thread.
//!
use align::Alignment;
use anyhow::{Context, Result};
use vscode_diff::LinesDiff;

pub mod contents;
pub mod runner;
pub mod worker;

pub use runner::{Diff, DiffContent, Runner, SingleFile};
pub use worker::{DiffWorker, Response};

/// User-visible options needed by the file diff pipeline.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Settings {
    pub ignore_trim_whitespace: bool,
}

/// Calls the C engine with application settings. Only for files with two sides.
pub fn compute(before: &[&str], after: &[&str], settings: Settings) -> Result<LinesDiff> {
    let options = vscode_diff::Options {
        ignore_trim_whitespace: settings.ignore_trim_whitespace,
        ..Default::default()
    };
    vscode_diff::compute(before, after, &options).context("computing the diff")
}

/// Pairs lines up from a diff result.
pub fn align(diff: LinesDiff, before: &[&str], after: &[&str]) -> Result<Alignment> {
    Alignment::try_new(diff, before, after)
        .map_err(|_| anyhow::anyhow!("the diff does not describe these two files"))
}

#[cfg(test)]
mod tests {
    use super::{Settings, compute};

    #[test]
    fn production_keeps_leading_and_trailing_whitespace_changes() {
        let diff = compute(&["  value"], &["value  "], Settings::default()).unwrap();

        assert!(!diff.is_empty());
    }
}
