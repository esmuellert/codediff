//! `codediff debug line <file>` — where each character of a line sits.
//!
//! Lists only the characters whose byte, UTF-16 and column positions differ.
//! Plain ASCII is skipped: for it all three are the same number, which is
//! precisely why nothing interesting happens there.

use anyhow::{Context, Result};
use line_index::{CellCol, DEFAULT_TAB_WIDTH, Grapheme, LineIndex};

use crate::text::{display_width, expand, pad, visible};

pub fn run(path: &str, verbose: bool) -> Result<()> {
    let text = std::fs::read_to_string(path).with_context(|| format!("reading {path}"))?;

    println!("{}", visible(path));
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
        let rows: Vec<Grapheme<'_>> = if verbose {
            line.graphemes().collect()
        } else {
            notable
        };
        // Size the label column to this line's widest character, so a lone
        // emoji does not sit eight columns from its numbers just because some
        // other line contains a joined sequence.
        let label_width = rows
            .iter()
            .map(|g| display_width(&name(g)))
            .max()
            .unwrap_or(0)
            .max(2);

        let last = rows.len().saturating_sub(1);
        for (i, g) in rows.iter().enumerate() {
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

/// Characters where the coordinate systems part company. A one-byte,
/// one-column ASCII character teaches nothing.
///
/// Control characters are included even though their coordinates agree: a byte
/// that draws nothing, or that the terminal would obey, is precisely what a
/// reviewer needs pointed out.
fn is_notable(g: &Grapheme<'_>) -> bool {
    g.is_tab() || g.width != 1 || g.text.len() != 1 || g.text.chars().any(char::is_control)
}

fn name(g: &Grapheme<'_>) -> String {
    // A label of "tab" would be indistinguishable from a line whose text is
    // the word "tab", which fixture line 13 actually is. These are the
    // symbols editors use when displaying whitespace.
    if g.is_tab() {
        return "⇥".to_owned();
    }
    if g.text == " " {
        return "␣".to_owned();
    }
    // A ZWJ sequence printed raw would look like a single emoji and hide why
    // its byte count is so large, so show its joiners.
    if g.text.chars().count() > 2 {
        return g
            .text
            .chars()
            .map(|c| match c {
                '\u{200d}' => "+".to_owned(),
                '\u{fe0f}' => String::new(),
                c => visible(&c.to_string()),
            })
            .collect();
    }
    visible(g.text)
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
