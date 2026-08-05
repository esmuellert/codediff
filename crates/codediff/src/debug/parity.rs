//! `codediff debug parity` — everything drawn, in a form a machine can diff.
//!
//! The other debug commands are for a reader. This one is for the harness that
//! checks us against `codediff.nvim`: the same records in the same order, so a
//! disagreement is a line of text rather than two screenshots.
//!
//! ```text
//! lines   <original count> <modified count>
//! filler  <side> <before this 1-based line> <count>
//! line    <side> <1-based line>                  a whole line marked changed
//! char    <side> <line> <start byte> <end byte>  a run of characters marked
//! ```

use std::path::Path;

use align::{Alignment, DiffLayout, ViewLineType};
use anyhow::{Context, Result};
use file_types::DiffVersion;

pub fn run(original_path: &str, modified_path: &str) -> Result<()> {
    let original_text = read(original_path)?;
    let modified_text = read(modified_path)?;
    let original = vscode_diff::lines(&original_text);
    let modified = vscode_diff::lines(&modified_text);

    let options = vscode_diff::Options::default().with_moves();
    let diff =
        vscode_diff::compute(&original, &modified, &options).context("computing the diff")?;
    let alignment = Alignment::new(diff, &original, &modified);

    println!("lines {} {}", original.len(), modified.len());
    fillers(&alignment);
    marked(&alignment);
    Ok(())
}

/// Where the fillers go, as "before this line, this many".
fn fillers(alignment: &Alignment) {
    for (side, version) in [
        ("original", DiffVersion::Original),
        ("modified", DiffVersion::Modified),
    ] {
        let mut run = 0;
        let mut last = 0;
        for line in alignment.view_lines(DiffLayout::SideBySide) {
            let slot = match version {
                DiffVersion::Original => line.original,
                DiffVersion::Modified => line.modified,
            };
            match slot.line() {
                Some(number) => {
                    if run > 0 {
                        println!("filler {side} {number} {run}");
                        run = 0;
                    }
                    last = number;
                }
                None => run += 1,
            }
        }
        if run > 0 {
            println!("filler {side} {} {run}", last + 1);
        }
    }
}

/// Which lines and which characters are marked as changed.
fn marked(alignment: &Alignment) {
    for (side, version) in [
        ("original", DiffVersion::Original),
        ("modified", DiffVersion::Modified),
    ] {
        for view_line in alignment.view_lines(DiffLayout::SideBySide) {
            if view_line.kind == ViewLineType::Unchanged {
                continue;
            }
            let slot = match version {
                DiffVersion::Original => view_line.original,
                DiffVersion::Modified => view_line.modified,
            };
            let Some(number) = slot.line() else { continue };
            println!("line {side} {number}");
            for span in alignment.spans(version, number) {
                println!(
                    "char {side} {number} {} {}",
                    span.bytes.start, span.bytes.end
                );
            }
        }
    }
}

fn read(path: &str) -> Result<String> {
    std::fs::read_to_string(Path::new(path)).with_context(|| format!("reading {path}"))
}
