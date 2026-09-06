//! Prints two files as aligned rows.

use ::align::{Alignment, DiffVersion, Slot, ViewLine, ViewLineType};
use anyhow::{Context, Result};
use file_types::DiffType;

use crate::text::{expand_str, fit, pad, sanitize};

/// Columns given to each file's text.
const COLUMN: u32 = 44;

pub fn run(original_path: &str, modified_path: &str, verbose: bool) -> Result<()> {
    let original_text = read(original_path)?;
    let modified_text = read(modified_path)?;
    let original = vscode_diff::lines(&original_text);
    let modified = vscode_diff::lines(&modified_text);

    // Include move detection.
    let options = vscode_diff::Options::default().with_moves();
    let diff =
        vscode_diff::compute(&original, &modified, &options).context("computing the diff")?;
    let alignment = Alignment::new(diff, &original, &modified);

    header(
        original_path,
        modified_path,
        &original,
        &modified,
        &alignment,
    );
    print(&alignment, verbose);
    Ok(())
}

/// Prints aligned rows and optional details.
pub fn print(alignment: &Alignment, verbose: bool) {
    for line in alignment.view_lines(DiffType::SideBySide) {
        println!("{}", rendered(alignment, &line));
    }
    if verbose {
        detail(alignment);
    }
}

fn header(
    original_path: &str,
    modified_path: &str,
    original: &[&str],
    modified: &[&str],
    alignment: &Alignment,
) {
    println!(
        "{}  ->  {}",
        sanitize(original_path),
        sanitize(modified_path)
    );
    println!(
        "{} lines -> {} lines, {} view lines",
        original.len(),
        modified.len(),
        alignment.view_line_count(DiffType::SideBySide)
    );
    println!(
        "{} change(s), {} move(s), {} hunk(s){}",
        alignment.changes().len(),
        alignment.moves().len(),
        alignment.hunks().len(),
        if alignment.hit_timeout() {
            "  [TIMED OUT]"
        } else {
            ""
        }
    );
    println!();
}

/// One view line, as text: line number, marker and content for each side.
fn rendered(alignment: &Alignment, line: &ViewLine) -> String {
    let (left_mark, right_mark) = marks(line.kind);
    let body = format!(
        "{} {} {} │ {} {} {}",
        number(line.original),
        left_mark,
        cell(alignment, DiffVersion::Original, line.original),
        number(line.modified),
        right_mark,
        pad(
            &cell(alignment, DiffVersion::Modified, line.modified),
            COLUMN
        ),
    );
    format!("{body}{}", move_note(alignment, line))
        .trim_end()
        .to_owned()
}

/// Formats a move marker at the start of a moved block.
fn move_note(alignment: &Alignment, line: &ViewLine) -> String {
    if let Some(n) = line.original.line()
        && let Some(moved) = alignment.moved(DiffVersion::Original, n)
        && moved.original.start_line == n
    {
        return format!("   ↓ moved to modified {}", moved.modified.start_line);
    }
    if let Some(n) = line.modified.line()
        && let Some(moved) = alignment.moved(DiffVersion::Modified, n)
        && moved.modified.start_line == n
    {
        return format!("   ↑ moved from original {}", moved.original.start_line);
    }
    String::new()
}

fn marks(kind: ViewLineType) -> (char, char) {
    match kind {
        ViewLineType::Unchanged => (' ', ' '),
        ViewLineType::Modified => ('~', '~'),
        ViewLineType::Deleted => ('-', ' '),
        ViewLineType::Inserted => (' ', '+'),
    }
}

fn number(slot: Slot) -> String {
    match slot.line() {
        Some(n) => format!("{n:>5}"),
        None => "     ".to_owned(),
    }
}

/// One version's cell on one line: its text, or fillers where it has no line.
fn cell(alignment: &Alignment, version: DiffVersion, slot: Slot) -> String {
    match slot.line() {
        None => "╱".repeat(COLUMN as usize),
        Some(number) => fit(
            &expand_str(alignment.line(version, number).unwrap_or_default()),
            COLUMN,
        ),
    }
}

/// Everything the line grid cannot show.
fn detail(alignment: &Alignment) {
    println!();
    println!("hunks");
    for hunk in alignment.hunks() {
        println!(
            "  {:016x}  original {}..{}  modified {}..{}  ({} change(s))",
            hunk.id.0,
            hunk.original.start_line,
            hunk.original.end_line,
            hunk.modified.start_line,
            hunk.modified.end_line,
            hunk.changes.len()
        );
    }

    println!();
    println!("character changes");
    let mut any = false;
    for line in alignment.view_lines(DiffType::SideBySide) {
        if line.kind != ViewLineType::Modified {
            continue;
        }
        for (version, slot) in [
            (DiffVersion::Original, line.original),
            (DiffVersion::Modified, line.modified),
        ] {
            let Some(number) = slot.line() else { continue };
            for span in alignment.spans(version, number) {
                any = true;
                let text = alignment.line(version, number).unwrap_or_default();
                let piece = text
                    .get(span.bytes.start as usize..span.bytes.end as usize)
                    .unwrap_or("<not a character boundary>");
                println!(
                    "  {:<8} line {number:>4}  bytes {:>4}..{:<4} {:?}",
                    label(version),
                    span.bytes.start,
                    span.bytes.end,
                    sanitize(piece)
                );
            }
        }
    }
    if !any {
        println!("  none");
    }

    if !alignment.moves().is_empty() {
        println!();
        println!("moves");
        for moved in alignment.moves() {
            println!(
                "  original {}..{}  ->  modified {}..{}",
                moved.original.start_line,
                moved.original.end_line,
                moved.modified.start_line,
                moved.modified.end_line
            );
        }
    }
}

fn label(version: DiffVersion) -> &'static str {
    match version {
        DiffVersion::Original => "original",
        DiffVersion::Modified => "modified",
    }
}

fn read(path: &str) -> Result<String> {
    std::fs::read_to_string(path).with_context(|| format!("reading {path}"))
}
