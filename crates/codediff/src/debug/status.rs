//! Prints working-tree status using the reviewer's file format.

use anyhow::{Context, Result};
use file_types::ChangeType;
use vcs::{DiffType, Repository};

use crate::text::{pad, sanitize};

pub fn run(dir: &str, verbose: bool) -> Result<()> {
    let mut repository = Repository::open(std::path::Path::new(dir))
        .with_context(|| format!("opening a repository at {dir}"))?;

    let repo = repository.repo_path().clone();
    println!("root     {}", sanitize(&repo.root.display().to_string()));
    println!(
        "git dir  {}",
        sanitize(&repo.control_dir.display().to_string())
    );

    let changed = repository
        .get_changed_files(&DiffType::Worktree, &[])
        .context("reading what changed")?;
    println!();
    if changed.is_empty() {
        println!("working tree clean");
        return Ok(());
    }

    // Group entries by their revision pair.
    let mut groups: Vec<(file_types::Revs, Vec<&file_types::File>)> = Vec::new();
    for file in &changed {
        let revs = file.revs();
        match groups.iter_mut().find(|(seen, _)| *seen == revs) {
            Some((_, files)) => files.push(file),
            None => groups.push((revs, vec![file])),
        }
    }

    for (revs, mut files) in groups {
        // Print the revision pair represented by this group.
        println!(
            "{} ({}) {} -> {}",
            revs.heading(),
            files.len(),
            revs.before,
            revs.after
        );
        files.sort_by(|a, b| a.path().as_str().cmp(b.path().as_str()));
        for file in files {
            println!("  {}", line(file, verbose));
        }
        println!();
    }
    Ok(())
}

/// Returns Git's status letter for a change.
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
fn line(file: &file_types::File, verbose: bool) -> String {
    let mut out = format!("{}  ", letter(file.get_change_type()));
    if verbose {
        // Pad by display columns.
        out.push_str(&pad(&sanitize(file.path().as_str()), 28));
    } else {
        out.push_str(&sanitize(file.path().as_str()));
    }

    if let Some(previous) = file
        .path_of_version(file_types::DiffVersion::Original)
        .filter(|original| original.as_str() != file.path().as_str())
    {
        out.push_str(&format!(" <- {}", sanitize(previous.as_str())));
    }
    if verbose {
        let note = match file.get_change_type() {
            ChangeType::Conflicted => "unresolved merge — listed, not diffable as two sides",
            ChangeType::Moved => "moved; both paths kept, not an add plus a delete",
            ChangeType::Untracked => "untracked — no before side to compare against",
            ChangeType::Added => "added",
            ChangeType::Deleted => "deleted",
            ChangeType::Modified => "modified",
        };
        out.push_str("  ");
        out.push_str(note);
    }
    out
}
