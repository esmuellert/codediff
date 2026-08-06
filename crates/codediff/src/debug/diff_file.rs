//! `codediff debug diff-file <path>` — one file, both sides found through git.
//!
//! The text-mode twin of the interface. Both go through the same pipeline, so
//! a disagreement between what this prints and what the screen shows would
//! have to come from drawing, not from the data.

use align::Comparison;
use anyhow::Result;
use file_types::ChangedFile;
use file_types::{DiffVersion, FileContent};

use crate::text::visible;
use pipeline::file::Runner;
use pipeline::list;

/// The file at `path`, as the list found it.
///
/// The list pipeline is the search, narrowed by a pathspec — the same one the
/// interface uses, so this prints what the screen would show. A path in two
/// groups gives two; the first is what is on disk, which is what a reader
/// naming a file means.
///
/// A file that has *not* changed is in no group at all, and is compared with
/// itself. That is a debugging answer rather than a review: the interface
/// refuses it, because a screen of unmarked text says nothing.
fn find(path: &str) -> Result<file_types::ChangedFile> {
    let cwd = std::env::current_dir()?;
    let git = vcs::Git::open(&cwd)?;
    let root = git.repo().root.clone();
    let request =
        explorer::ExplorerDiffRequest::worktree(root.clone()).with_pathspec(vec![path.to_owned()]);

    if let Some(file) = list::files(&request)?.into_iter().next() {
        return Ok(file);
    }
    let wanted = file_types::RepoPath::new(path, &root);
    if wanted.as_path().exists() {
        let mut git = git;
        let revs = git.revs()?;
        return Ok(ChangedFile::new(
            file_types::File::unchanged_path(wanted, revs),
            None,
        ));
    }
    anyhow::bail!("{path} is neither changed nor present")
}

pub fn run(path: &str, verbose: bool) -> Result<()> {
    let runner = Runner::new(&find(path)?)?;
    let contents = &runner.contents;
    header(&contents.diff, &contents.original, &contents.modified);

    // Nothing to align: a picture has no lines, and saying so is the answer
    // rather than a failure.
    if runner.is_binary() {
        println!("binary file — no line diff");
        return Ok(());
    }

    // A file that exists on only one side is not compared against anything, so
    // there is no diff to print — only the file. This is what the interface
    // shows too, in one pane rather than two.
    if let Some(version) = runner.only() {
        return one_sided(&runner, version);
    }

    // The same comparison the interface is given, read rather than drawn. Any
    // disagreement between this and the screen would have to come from
    // drawing, since there is only one source for both.
    let comparison = runner.run()?;
    let Comparison::Both { alignment, .. } = &comparison else {
        unreachable!("two sides were read, so this is a diff");
    };
    println!(
        "{} line(s) -> {} line(s), {} view line(s), {} change(s)",
        alignment.lines(DiffVersion::Original).len(),
        alignment.lines(DiffVersion::Modified).len(),
        alignment.view_line_count(::align::DiffLayout::SideBySide),
        alignment.changes().len()
    );
    println!();
    super::print_alignment(alignment, verbose);
    Ok(())
}

/// Prints the one side that exists, numbered and unmarked.
///
/// No `+` or `-`: nothing here changed relative to anything, because there is
/// no other side to be relative to.
fn one_sided(runner: &Runner, present: DiffVersion) -> Result<()> {
    let what = match present {
        DiffVersion::Modified => "added — no original to compare against",
        DiffVersion::Original => "deleted — showing what was removed",
    };
    let numbered = runner.contents.version(present);
    println!("{} line(s), {what}", numbered.len());
    println!();
    for (i, line) in numbered.iter().enumerate() {
        println!("{:>5}   {}", i + 1, visible(line));
    }
    Ok(())
}

fn header(diff: &ChangedFile, original: &FileContent, modified: &FileContent) {
    println!("{}", visible(diff.path().as_str()));
    if let Some(previous) = diff.file.previous_path() {
        println!("moved from {}", visible(previous.as_str()));
    }
    println!("{:?}", diff.change());
    println!();
    println!("before   {}", original.describe());
    println!("after    {}", modified.describe());
    println!();
}
