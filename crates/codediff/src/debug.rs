//! `codediff debug diff <a> <b>` — the raw diff, as text.
//!
//! Exists for three reasons: it makes a headless milestone checkable by a
//! human, it turns a bug report into "send me this output", and it keeps the
//! layering honest, since a command that can drive `vscode-diff` on its own
//! proves the crate does not need the rest of the application to be useful.

use anyhow::{Context, Result};
use std::path::Path;
use vscode_diff::{DetailedLineRangeMapping, LinesDiff, Options};

pub fn run(original_path: &str, modified_path: &str) -> Result<()> {
    let original_text = read(original_path)?;
    let modified_text = read(modified_path)?;

    // Split on '\n' only, keeping a trailing empty line, which is how the
    // engine and JavaScript both model a file. `str::lines` discards the
    // trailing empty line and strips '\r', which would shift every range.
    let original: Vec<&str> = original_text.split('\n').collect();
    let modified: Vec<&str> = modified_text.split('\n').collect();

    let options = Options::default().with_moves();
    let diff = vscode_diff::compute(&original, &modified, &options)?;

    println!("original  {original_path} ({} lines)", original.len());
    println!("modified  {modified_path} ({} lines)", modified.len());
    println!("engine    libvscode-diff {}", vscode_diff::engine_version());
    println!();
    report(&diff);
    Ok(())
}

fn report(diff: &LinesDiff) {
    if diff.is_empty() {
        println!("no changes");
        return;
    }

    println!(
        "{} change(s), {} move(s){}",
        diff.changes.len(),
        diff.moves.len(),
        if diff.hit_timeout {
            ", TIMED OUT (result is coarser than usual)"
        } else {
            ""
        }
    );
    println!();

    for (i, change) in diff.changes.iter().enumerate() {
        println!(
            "  [{i}] {:<9}  original {}  modified {}",
            kind(change),
            span(change.original.start_line, change.original.end_line),
            span(change.modified.start_line, change.modified.end_line),
        );
        for inner in &change.inner_changes {
            println!(
                "        inner  L{}:C{}-L{}:C{}  ->  L{}:C{}-L{}:C{}",
                inner.original.start_line,
                inner.original.start_col,
                inner.original.end_line,
                inner.original.end_col,
                inner.modified.start_line,
                inner.modified.start_col,
                inner.modified.end_line,
                inner.modified.end_col,
            );
        }
    }

    if !diff.moves.is_empty() {
        println!();
        println!("  moves");
        for (i, moved) in diff.moves.iter().enumerate() {
            println!(
                "  [{i}] original {}  ->  modified {}",
                span(moved.original.start_line, moved.original.end_line),
                span(moved.modified.start_line, moved.modified.end_line),
            );
        }
    }

    println!();
    println!("line ranges are 1-based and end-exclusive; columns are UTF-16 code units");
}

fn kind(change: &DetailedLineRangeMapping) -> &'static str {
    if change.is_insertion() {
        "inserted"
    } else if change.is_deletion() {
        "deleted"
    } else {
        "modified"
    }
}

/// Renders a range, marking the empty ones so that an insertion or deletion
/// point is not mistaken for a typo.
fn span(start: u32, end: u32) -> String {
    if start >= end {
        format!("{start}..{end} (empty)")
    } else {
        format!("{start}..{end}")
    }
}

fn read(path: &str) -> Result<String> {
    std::fs::read_to_string(Path::new(path)).with_context(|| format!("reading {path}"))
}
