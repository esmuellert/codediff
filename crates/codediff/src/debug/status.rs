//! `codediff debug status` — what the working tree looks like.
//!
//! What the repository says, in the reviewer's terms, because that is the only
//! vocabulary above `vcs`. Git's own `XY` codes are checked against the
//! fixture manifest inside `vcs`, beside the parser that reads them — a
//! subcommand cannot see them any more, which is the point of D67.

use anyhow::{Context, Result};
use file_types::ChangeType;
use vcs::{DiffType, Repository};

use crate::text::{pad, visible};

pub fn run(dir: &str, verbose: bool) -> Result<()> {
    let mut repository = Repository::open(std::path::Path::new(dir))
        .with_context(|| format!("opening a repository at {dir}"))?;

    let repo = repository.repo().clone();
    println!("root     {}", visible(&repo.root.display().to_string()));
    println!(
        "git dir  {}",
        visible(&repo.control_dir.display().to_string())
    );

    let groups = repository
        .changes(&DiffType::Worktree, &[])
        .context("reading what changed")?;
    println!();
    if groups.iter().all(|group| group.files.is_empty()) {
        println!("working tree clean");
        return Ok(());
    }

    for group in &groups {
        if group.files.is_empty() {
            continue;
        }
        // The revisions, not only the name: a name is a label a human reads,
        // and what the group *is* is the pair.
        println!(
            "{} ({}) {} -> {}",
            group.revs.heading(),
            group.files.len(),
            group.revs.before,
            group.revs.after
        );
        let mut files: Vec<_> = group.files.iter().collect();
        files.sort_by(|a, b| a.path().as_str().cmp(b.path().as_str()));
        for file in files {
            println!("  {}", line(file, verbose));
        }
        println!();
    }
    Ok(())
}

/// Git's letter for what happened, as `git status` prints it.
///
/// Not the interface's: that one is beside the theme that colours it, and
/// spells an untracked file `??` because a column of them reads better. This
/// is a debug command echoing git.
pub fn letter(change: ChangeType) -> &'static str {
    match change {
        ChangeType::Added => "A",
        ChangeType::Modified => "M",
        ChangeType::Deleted => "D",
        ChangeType::Moved => "R",
        ChangeType::Untracked => "?",
        ChangeType::Conflicted => "U",
    }
}

/// `X  path [<- original]`, one line per file.
fn line(file: &file_types::ChangedFile, verbose: bool) -> String {
    let mut out = format!("{}  ", letter(file.change()));
    if verbose {
        // Padded by display columns, not characters: a CJK filename is twice
        // as wide as its character count suggests.
        out.push_str(&pad(&visible(file.path().as_str()), 28));
    } else {
        out.push_str(&visible(file.path().as_str()));
    }

    if let Some(previous) = file
        .file
        .on(file_types::DiffVersion::Original)
        .filter(|original| original.as_str() != file.path().as_str())
    {
        out.push_str(&format!(" <- {}", visible(previous.as_str())));
    }
    if verbose {
        let note = match file.change() {
            ChangeType::Conflicted => "unresolved merge — listed, not diffable as two sides",
            ChangeType::Moved => "moved; both paths kept, not an add plus a delete",
            ChangeType::Untracked => "untracked — no before side to compare against",
            ChangeType::Added => "added",
            ChangeType::Deleted => "deleted",
            ChangeType::Modified => "modified",
        };
        out.push_str(note);
        if let Some(similarity) = file.similarity {
            out.push_str(&format!(" ({similarity}% similar)"));
        }
    }
    out
}
