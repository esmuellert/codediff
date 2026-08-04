//! `codediff debug diff-file <path>` — one file, both sides found through git.
//!
//! The text-mode twin of the interface. Both go through the same pipeline, so
//! a disagreement between what this prints and what the screen shows would
//! have to come from drawing, not from the data.

use anyhow::Result;
use file_types::ChangedFile;
use file_types::{DiffVersion, FileContent};

use crate::pipeline::{Request, Runner};
use crate::text::visible;

pub fn run(path: &str, verbose: bool) -> Result<()> {
    let runner = Runner::new(&Request::Worktree { path })?;
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

    // The same buffer the interface is given, read rather than drawn. Any
    // disagreement between this and the screen would have to come from
    // drawing, since there is only one source for both.
    let buffer = runner.run()?;
    let Some(alignment) = buffer.alignment() else {
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
