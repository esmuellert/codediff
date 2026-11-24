//! Prints the groups and files produced by a list request.

use anyhow::{Context, Result};
use vcs::RevisionRange;

use crate::cli::parse_revision;

/// Converts list flags and revisions into a VCS comparison type.
pub fn revision_range(rev: &[String], staged: bool) -> RevisionRange {
    match (staged, rev.first().cloned(), rev.get(1).cloned()) {
        // `--staged` with no revision means against the last commit, which is
        // what `git diff --cached` means with no revision.
        (true, rev, _) => RevisionRange::Staged(rev.unwrap_or_else(|| "HEAD".to_owned())),
        (false, None, _) => RevisionRange::Worktree,
        (false, Some(a), Some(b)) => RevisionRange::Between(a, b),
        (false, Some(a), None) => parse_revision(&a),
    }
}

/// Prints each revision group and its files.
pub fn run(revision_range: RevisionRange, pathspec: Vec<String>) -> Result<()> {
    let cwd = std::env::current_dir()?;
    let mut repository = vcs::Repository::open(&cwd).context("opening a repository")?;
    let changed_files = repository
        .get_changed_files(&revision_range, &pathspec)
        .context("listing changed files and line counts")?;

    let mut groups: Vec<(file_types::Revs, Vec<file_types::File>)> = Vec::new();
    for file in changed_files {
        let revs = file.revs();
        match groups.iter_mut().find(|(seen, _)| *seen == revs) {
            Some((_, files)) => files.push(file),
            None => groups.push((revs, vec![file])),
        }
    }

    for (revs, files) in groups {
        // Print the revision pair represented by this group.
        println!(
            "group {:?} {} -> {}",
            revs.heading(),
            revs.before,
            revs.after
        );
        for file in &files {
            let stats = match file.get_stats() {
                Some(stats) => format!(" +{} -{}", stats.added, stats.removed),
                None => String::new(),
            };
            println!(
                "  {} {}{stats}",
                super::status::letter(file.get_change_type()),
                file.path()
            );
        }
    }
    Ok(())
}
