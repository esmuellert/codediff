//! `codediff debug status` — what git says about the working tree.
//!
//! The format matches the fixture manifest, so the acceptance check for S5 is
//! reading the two side by side.

use anyhow::{Context, Result};
use vcs::git::Entry;
use vcs::{Diff, DiffKind, FileDiff, Git};

use crate::text::{pad, visible};

pub fn run(dir: &str, verbose: bool) -> Result<()> {
    let git = Git::open(std::path::Path::new(dir))
        .with_context(|| format!("opening a repository at {dir}"))?;

    let repo = git.repo().clone();
    println!("root     {}", visible(&repo.root.display().to_string()));
    println!(
        "git dir  {}",
        visible(&repo.control_dir.display().to_string())
    );

    // Git's own records, because the manifest this is checked against is
    // written in git's XY spelling.
    let entries = git.entries().context("reading status")?;
    println!();
    if entries.is_empty() {
        println!("working tree clean");
        return Ok(());
    }

    println!("{} entry(s)   index worktree path", entries.len());
    println!();
    let mut sorted: Vec<&Entry> = entries.iter().collect();
    sorted.sort_by(|a, b| a.path.as_str().cmp(b.path.as_str()));
    for entry in &sorted {
        println!("  {}", line(entry));
    }

    if verbose {
        detail(&sorted);
    }
    Ok(())
}

/// `X  Y  path [<- original]`, the manifest's own shape.
fn line(entry: &Entry) -> String {
    let mut out = format!(
        "{}  {}  {}",
        entry.xy.index.letter(),
        entry.xy.worktree.letter(),
        visible(entry.path.as_str())
    );
    if let Some(original) = &entry.original {
        out.push_str(&format!(" <- {}", visible(original.as_str())));
    }
    out
}

/// The same entries as the reviewer sees them, after the one translation from
/// git's model to ours.
fn detail(entries: &[&Entry]) {
    println!();
    println!("as the reviewer sees them");
    for entry in entries {
        let file: FileDiff = vcs::git::to_file_diff((*entry).clone());
        let note = match file.kind {
            DiffKind::Conflicted => "unresolved merge — listed, not diffable as two sides",
            DiffKind::Moved => "moved; both paths kept, not an add plus a delete",
            DiffKind::Untracked => "untracked — no before side to compare against",
            DiffKind::Added => "added",
            DiffKind::Deleted => "deleted",
            DiffKind::Modified => "modified",
        };
        let similarity = file
            .similarity
            .map(|s| format!(" ({s}% similar)"))
            .unwrap_or_default();
        // Padded by display columns, not characters: a CJK filename is twice
        // as wide as its character count suggests.
        println!(
            "  {} {note}{similarity}",
            pad(&visible(file.path.as_str()), 28)
        );
    }
}
