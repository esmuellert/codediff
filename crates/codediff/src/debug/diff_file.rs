//! Prints one repository file with both versions.

use anyhow::Result;
use file_types::File;
use file_types::{DiffVersion, FileContent};
use pipeline::diff::DiffContent;

use crate::text::sanitize;
use pipeline::diff::Runner;
use pipeline::files;

/// Finds a changed file, or an existing file for self-comparison.
fn find(path: &str) -> Result<file_types::File> {
    let cwd = std::env::current_dir()?;
    let git = vcs::Repository::open(&cwd)?;
    let root = git.repo_path().root.clone();
    // Search the full list so either side of a rename can match.
    let request = pipeline::files::Request::worktree(root.clone());
    let listed = files::get_files(&request)?.into_iter().find(|file| {
        file.path().as_str() == path || file.previous_path().is_some_and(|was| was.as_str() == path)
    });
    if let Some(file) = listed {
        return Ok(file);
    }
    let repo_path = file_types::RepoPath::new(path, &root);
    if repo_path.as_path().exists() {
        // Use identical revisions for an unchanged file.
        let revs = file_types::Revs::worktree_against(file_types::Oid::new("HEAD"));
        return Ok(file_types::File::unchanged_path(repo_path, revs));
    }
    anyhow::bail!("{path} is neither changed nor present")
}

pub fn run(path: &str, verbose: bool) -> Result<()> {
    let runner = Runner::new(&find(path)?)?;
    let contents = &runner.contents;
    header(&contents.file, &contents.original, &contents.modified);

    // Binary files have no line diff.
    if runner.is_binary() {
        println!("binary file — no line diff");
        return Ok(());
    }

    // One-sided files are shown in one pane.
    if let Some(version) = runner.is_one_sided() {
        return one_sided(&runner, version);
    }

    // Print the aligned diff.
    let content = runner.compute_diff()?;
    let DiffContent::Diff(diff) = &content else {
        unreachable!("two sides were read, so this is a diff");
    };
    let alignment = &diff.alignment;
    println!(
        "{} line(s) -> {} line(s), {} view line(s), {} change(s)",
        alignment.lines(DiffVersion::Original).len(),
        alignment.lines(DiffVersion::Modified).len(),
        alignment.view_line_count(file_types::DiffType::SideBySide),
        alignment.changes().len()
    );
    println!();
    super::print_alignment(alignment, verbose);
    Ok(())
}

/// Prints the one side that exists, numbered and unmarked.
///
/// Prints the existing side with line numbers.
fn one_sided(runner: &Runner, present: DiffVersion) -> Result<()> {
    let what = match present {
        DiffVersion::Modified => "added — no original to compare against",
        DiffVersion::Original => "deleted — showing what was removed",
    };
    let numbered = runner.contents.version(present);
    println!("{} line(s), {what}", numbered.len());
    println!();
    for (i, line) in numbered.iter().enumerate() {
        println!("{:>5}   {}", i + 1, sanitize(line));
    }
    Ok(())
}

fn header(diff: &File, original: &FileContent, modified: &FileContent) {
    println!("{}", sanitize(diff.path().as_str()));
    if let Some(previous) = diff.previous_path() {
        println!("moved from {}", sanitize(previous.as_str()));
    }
    println!("{:?}", diff.get_change_type());
    println!();
    println!("before   {}", original.describe());
    println!("after    {}", modified.describe());
    println!();
}
