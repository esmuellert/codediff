//! `codediff debug list` — the groups a request produces.
//!
//! The list pipeline, printed. `debug status` shows what git said; this shows
//! what the explorer will be handed, which is the other side of the
//! translation and the only place the two can be compared.
//!
//! Machine-readable on purpose: one line per group and one per file, so a test
//! can assert on it without a terminal.

use anyhow::Result;
use explorer::{ExplorerDiffRequest, ExplorerDiffType};

use crate::pipeline;

/// Prints every group, and every file in it.
pub fn run(diff_type: ExplorerDiffType, pathspec: Vec<String>) -> Result<()> {
    let cwd = std::env::current_dir()?;
    let root = vcs::Git::open(&cwd)?.repo().root.clone();
    let request = ExplorerDiffRequest::new(root, diff_type).with_pathspec(pathspec);

    for group in pipeline::list::run(&request)? {
        // The revisions, not only the name: a name is a label a human reads,
        // and what the group *is* is the pair.
        println!(
            "group {:?} {} -> {}",
            group.name, group.revs.before, group.revs.after
        );
        for entry in &group.files {
            let stats = match entry.stats {
                Some(stats) => format!(" +{} -{}", stats.added, stats.removed),
                None => String::new(),
            };
            println!("  {} {}{stats}", entry.status(), entry.path());
        }
    }
    Ok(())
}
