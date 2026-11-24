//! Prints one repository file with both versions.

use anyhow::{Context, Result};
use diff::{DiffContent, Settings, SingleFile};
use file_types::{CommitHash, DiffVersion, File, FileContent, RepoPath, Revs};
use vcs::{Repository, RevisionRange};

use crate::text::sanitize;

/// Finds a changed file, or an existing file for self-comparison.
fn find(repository: &mut Repository, path: &str) -> Result<File> {
    let root = repository.repo_path().root.clone();
    // Search the full list so either side of a rename can match.
    let listed = repository
        .get_changed_files(&RevisionRange::Worktree, &[])
        .context("listing changed files and line counts")?
        .into_iter()
        .find(|file| {
            file.path().as_str() == path
                || file.previous_path().is_some_and(|was| was.as_str() == path)
        });
    if let Some(file) = listed {
        return Ok(file);
    }
    let repo_path = RepoPath::new(path, &root);
    if repo_path.as_path().exists() {
        // Use identical revisions for an unchanged file.
        let revs = Revs::worktree_against(CommitHash::new("HEAD"));
        return Ok(File::unchanged_path(repo_path, revs));
    }
    anyhow::bail!("{path} is neither changed nor present")
}

pub fn run(path: &str, verbose: bool) -> Result<()> {
    let cwd = std::env::current_dir()?;
    let mut repository = Repository::open(&cwd).context("opening a repository")?;
    let file = find(&mut repository, path)?;
    let original = repository
        .get_file_content(&file, DiffVersion::Original)
        .context("reading the before side")?;
    let modified = repository
        .get_file_content(&file, DiffVersion::Modified)
        .context("reading the after side")?;
    header(&file, &original, &modified);

    // Binary files have no line diff.
    if original.is_binary() || modified.is_binary() {
        println!("binary file — no line diff");
        return Ok(());
    }

    let content = DiffContent::new(file, &original, &modified, Settings::default())?;
    match &content {
        DiffContent::SingleFile(single) => one_sided(single),
        DiffContent::Diff(diff) => {
            let alignment = &diff.alignment;
            println!(
                "{} line(s) -> {} line(s), {} view line(s), {} change(s)",
                alignment.lines(DiffVersion::Original).len(),
                alignment.lines(DiffVersion::Modified).len(),
                alignment.view_line_count(file_types::ViewerMode::SideBySide),
                alignment.changes().len()
            );
            println!();
            super::print_alignment(alignment, verbose);
            Ok(())
        }
    }
}

/// Prints the existing side with line numbers.
fn one_sided(single: &SingleFile) -> Result<()> {
    let what = match single.side() {
        DiffVersion::Modified => "added — no original to compare against",
        DiffVersion::Original => "deleted — showing what was removed",
    };
    println!("{} line(s), {what}", single.lines.len());
    println!();
    for (i, line) in single.lines.iter().enumerate() {
        println!("{:>5}   {}", i + 1, sanitize(line));
    }
    Ok(())
}

fn header(file: &File, original: &FileContent, modified: &FileContent) {
    println!("{}", sanitize(file.path().as_str()));
    if let Some(previous) = file.previous_path() {
        println!("moved from {}", sanitize(previous.as_str()));
    }
    println!("{:?}", file.get_change_type());
    println!();
    println!("before   {}", original.describe());
    println!("after    {}", modified.describe());
    println!();
}
