//! Prints byte, UTF-16, and terminal-cell positions.

use anyhow::{Context, Result};
use line_index::{CellCol, DEFAULT_TAB_WIDTH, Grapheme, LineIndex};

use crate::text::{display_width, expand, pad, sanitize};

pub fn run(path: &str, verbose: bool) -> Result<()> {
    let text = std::fs::read_to_string(path).with_context(|| format!("reading {path}"))?;

    println!("{}", sanitize(path));
    println!();
    if verbose {
        println!("Every character, with a cell ruler under each line:");
    } else {
        println!("Characters where the coordinate systems disagree, plus controls:");
    }
    println!();

    let mut plain_lines = 0;
    let mut total = 0;

    for (number, raw) in text.lines().enumerate() {
        total += 1;
        let line = LineIndex::new(raw, DEFAULT_TAB_WIDTH);
        let notable: Vec<Grapheme<'_>> = line.graphemes().filter(is_notable).collect();

        if notable.is_empty() && !verbose {
            plain_lines += 1;
            continue;
        }

        println!("  line {:<3} \"{}\"", number + 1, expand(&line));
        let lines: Vec<Grapheme<'_>> = if verbose {
            line.graphemes().collect()
        } else {
            notable
        };
        // Align labels to this line's widest grapheme.
        let label_width = lines
            .iter()
            .map(|g| display_width(&name(g)))
            .max()
            .unwrap_or(0)
            .max(2);

        let last = lines.len().saturating_sub(1);
        for (i, g) in lines.iter().enumerate() {
            let connector = if i == last { "└─" } else { "├─" };
            println!(
                "    {connector} {}  byte {:>3}   utf16 {:>3}   column {:>3}   width {}",
                pad(&name(g), label_width),
                g.byte.get(),
                g.utf16.get(),
                g.cell.get(),
                g.width,
            );
        }

        if verbose {
            ruler_check(&line);
        }
        println!();
    }

    if plain_lines > 0 {
        println!(
            "  {plain_lines} of {total} lines are plain ASCII, where byte, utf16 and column are all equal."
        );
    }
    Ok(())
}

/// Returns graphemes with nontrivial coordinates or terminal controls.
fn is_notable(g: &Grapheme<'_>) -> bool {
    g.is_tab() || g.width != 1 || g.text.len() != 1 || g.text.chars().any(char::is_control)
}

fn name(g: &Grapheme<'_>) -> String {
    // Use visible labels for whitespace.
    if g.is_tab() {
        return "⇥".to_owned();
    }
    if g.text == " " {
        return "␣".to_owned();
    }
    // Show joiners in multi-codepoint graphemes.
    if g.text.chars().count() > 2 {
        return g
            .text
            .chars()
            .map(|c| match c {
                '\u{200d}' => "+".to_owned(),
                '\u{fe0f}' => String::new(),
                c => sanitize(&c.to_string()),
            })
            .collect();
    }
    sanitize(g.text)
}

/// A visual check that the computed widths match what the terminal draws:
/// `^` where a character starts, `-` for the columns it continues into.
fn ruler_check(line: &LineIndex<'_>) {
    let mut map = String::new();
    for g in line.graphemes() {
        if g.width > 0 {
            map.push('^');
            map.extend(std::iter::repeat_n('-', (g.width - 1) as usize));
        }
    }
    println!("         {}", expand(line));
    println!("         {map}");
    println!("         {}", ruler(line.width()));
}

fn ruler(width: CellCol) -> String {
    (0..width.get())
        .map(|cell| match cell % 10 {
            0 => char::from_digit((cell / 10) % 10, 10).unwrap_or('|'),
            5 => '+',
            _ => '·',
        })
        .collect()
}
