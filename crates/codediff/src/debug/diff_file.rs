//! `codediff debug diff-file <path>` — one file, both sides found through git.
//!
//! The first command that uses every layer at once: `vcs` finds the file and
//! reads its two sides, the C engine compares them, `align` pairs them up. The
//! rendering is `debug align`'s, so the two can be compared directly.

use align::Alignment;
use anyhow::{Context, Result, bail};
use vcs::{Content, Diff, DiffKind, FileDiff, Git, RelPath};

use crate::text::visible;
use vscode_diff::lines;

pub fn run(path: &str, verbose: bool) -> Result<()> {
    let cwd = std::env::current_dir().context("finding the current directory")?;
    let mut git = Git::open(&cwd).context("opening a repository")?;

    let file = find(&mut git, path)?;
    let before = git.before(&file).context("reading the before side")?;
    let after = git.after(&file).context("reading the after side")?;

    header(&file, &before, &after);

    // Nothing to align: a picture has no lines, and saying so is the answer
    // rather than a failure.
    if before.is_binary() || after.is_binary() {
        println!("binary file — no line diff");
        return Ok(());
    }

    // A side that does not exist has *no* lines. The engine cannot say that —
    // it models an empty file as one empty line — so comparing an absent side
    // against a real one would report the file as modified, with a phantom
    // blank line paired against its first real one. One-sided changes are
    // answered here instead.
    if let Some(()) = one_sided(&file, &before, &after) {
        return Ok(());
    }

    let before_lines = lines(before.or_empty());
    let after_lines = lines(after.or_empty());

    let options = vscode_diff::Options::default().with_moves();
    let diff = vscode_diff::compute(&before_lines, &after_lines, &options)
        .context("computing the diff")?;
    let alignment = Alignment::new(&diff, &before_lines, &after_lines);

    println!(
        "{} line(s) -> {} line(s), {} row(s), {} change(s)",
        before_lines.len(),
        after_lines.len(),
        alignment.row_count(),
        diff.changes.len()
    );
    println!();
    super::print_alignment(&alignment, verbose);
    Ok(())
}

/// Locates the file among those git reports as changed.
///
/// By path as given, then relative to the repository root, so it works from a
/// subdirectory the way git's own commands do.
fn find(git: &mut Git, path: &str) -> Result<FileDiff> {
    let files = git.files().context("listing changed files")?;
    let wanted = RelPath::new(path);

    if let Some(found) = files
        .iter()
        .find(|f| f.path == wanted || f.previous_path.as_ref() == Some(&wanted))
    {
        return Ok(found.clone());
    }

    // Not changed, but it may still exist — comparing a file with itself is a
    // legitimate thing to ask for, and produces an empty diff rather than an
    // error.
    let absolute = wanted.to_absolute(&git.repo().root);
    if absolute.exists() {
        return Ok(FileDiff {
            path: wanted,
            previous_path: None,
            kind: DiffKind::Modified,
            similarity: None,
        });
    }

    bail!(
        "{path} is neither changed nor present; git reports {} changed file(s)",
        files.len()
    )
}

/// Prints a file that exists on only one side, which is every line added or
/// every line removed.
fn one_sided(file: &FileDiff, before: &Content, after: &Content) -> Option<()> {
    let (text, mark, what) = match (before.text(), after.text()) {
        (None, Some(text)) => (text, '+', "every line is new"),
        (Some(text), None) => (text, '-', "every line is gone"),
        _ => return None,
    };
    let numbered = lines(text);
    println!("{} line(s), {what}", numbered.len());
    println!();
    for (i, line) in numbered.iter().enumerate() {
        println!("{:>5} {mark} {}", i + 1, visible(line));
    }
    let _ = file;
    Some(())
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
