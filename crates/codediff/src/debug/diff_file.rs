//! `codediff debug diff-file <path>` — one file, both sides found through git.
//!
//! The text-mode twin of the interface. Both go through the same pipeline, so
//! a disagreement between what this prints and what the screen shows would
//! have to come from drawing, not from the data.

use align::Side;
use anyhow::Result;
use vcs::{Content, FileDiff};

use crate::pipeline::{Request, Runner};
use crate::text::visible;

pub fn run(path: &str, verbose: bool) -> Result<()> {
    let runner = Runner::new(&Request::Worktree { path })?;
    let contents = &runner.contents;
    header(&contents.file, &contents.before, &contents.after);

    // Nothing to align: a picture has no lines, and saying so is the answer
    // rather than a failure.
    if runner.is_binary() {
        println!("binary file — no line diff");
        return Ok(());
    }

    // A file that exists on only one side is not compared against anything, so
    // there is no diff to print — only the file. This is what the interface
    // shows too, in one pane rather than two.
    if let Some(side) = runner.only() {
        return one_sided(&runner, side);
    }

    // The same buffer the interface is given, read rather than drawn. Any
    // disagreement between this and the screen would have to come from
    // drawing, since there is only one source for both.
    let ui::Buffer::SideBySide(data) = runner.run()? else {
        unreachable!("two sides were read, so this is a diff");
    };
    let alignment = data.alignment();
    println!(
        "{} line(s) -> {} line(s), {} row(s), {} change(s)",
        alignment.lines(Side::Original).len(),
        alignment.lines(Side::Modified).len(),
        alignment.row_count(),
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
fn one_sided(runner: &Runner, present: Side) -> Result<()> {
    let what = match present {
        Side::Modified => "added — no original to compare against",
        Side::Original => "deleted — showing what was removed",
    };
    let numbered = runner.contents.side(present);
    println!("{} line(s), {what}", numbered.len());
    println!();
    for (i, line) in numbered.iter().enumerate() {
        println!("{:>5}   {}", i + 1, visible(line));
    }
    Ok(())
}

fn header(file: &FileDiff, before: &Content, after: &Content) {
    println!("{}", visible(file.path.as_str()));
    if let Some(previous) = &file.previous_path {
        println!("moved from {}", visible(previous.as_str()));
    }
    println!("{:?}", file.kind);
    println!();
    println!("before   {}", before.describe());
    println!("after    {}", after.describe());
    println!();
}
