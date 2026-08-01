//! Turning file content into something safe to print.
//!
//! Shared by every `debug` subcommand. A terminal has one input stream and no
//! way to tell text apart from commands, so anything read out of a file under
//! review is neutralised before it reaches the screen — otherwise the file
//! decides what the reviewer sees.

use metrics::{DEFAULT_TAB_WIDTH, LineMetrics};

/// Text with anything the terminal would act on replaced by a printable
/// stand-in of the same width.
pub fn visible(text: &str) -> String {
    if !text.chars().any(is_dangerous) {
        return text.to_owned();
    }
    text.chars().map(picture).collect()
}

/// Characters a file must not be allowed to send to the terminal showing it.
///
/// Two families, for the same reason: both let a file decide what the reviewer
/// sees rather than what it contains.
///
/// - **Control characters.** `ESC` starts a sequence the terminal *obeys* —
///   recolour, move the cursor, erase what is already drawn.
/// - **Bidirectional formatting.** `U+202E RIGHT-TO-LEFT OVERRIDE` and the
///   isolates reorder a line on screen, so it reads as something other than
///   what it executes. This is the Trojan Source attack, and `char::is_control`
///   does not cover it: those are format characters, category `Cf`, not `Cc`.
fn is_dangerous(c: char) -> bool {
    c.is_control() || metrics::is_bidi_control(c)
}

fn picture(c: char) -> char {
    match c {
        // Unicode Control Pictures: U+2400 draws U+0000, U+2401 draws U+0001,
        // and so on through the C0 range.
        '\u{0}'..='\u{1f}' => char::from_u32(0x2400 + c as u32).unwrap_or('\u{fffd}'),
        '\u{7f}' => '\u{2421}', // DEL has its own picture
        // Neither the C1 controls nor the bidi characters have a picture, and
        // both must occupy a column so the ruler still lines up.
        c if is_dangerous(c) => '\u{fffd}',
        c => c,
    }
}

/// Tabs replaced by the spaces they expand to, and controls by their picture.
///
/// A raw tab would use the *terminal's* tab stops rather than the ones we
/// measured with, so the two would disagree about where anything sits.
pub fn expand(line: &LineMetrics<'_>) -> String {
    let mut out = String::with_capacity(line.text().len());
    for g in line.graphemes() {
        if g.is_tab() {
            out.extend(std::iter::repeat_n(' ', g.width as usize));
        } else {
            out.push_str(&visible(g.text));
        }
    }
    out
}

/// The same, straight from a string.
pub fn expand_str(text: &str) -> String {
    expand(&LineMetrics::new(text, DEFAULT_TAB_WIDTH))
}

/// Terminal columns the text occupies.
pub fn display_width(text: &str) -> u32 {
    LineMetrics::new(text, DEFAULT_TAB_WIDTH).width().get()
}

/// Pads to terminal columns rather than characters, so a double-width
/// character does not shift the rest of the row.
pub fn pad(text: &str, columns: u32) -> String {
    let mut out = text.to_owned();
    out.extend(std::iter::repeat_n(
        ' ',
        columns.saturating_sub(display_width(text)) as usize,
    ));
    out
}

/// Clips to a column count without splitting a character in half, then pads.
pub fn fit(text: &str, columns: u32) -> String {
    if columns == 0 {
        return String::new();
    }
    let width = display_width(text);
    if width <= columns {
        return pad(text, columns);
    }
    // Leave a column for the ellipsis, and stop before any cluster that would
    // straddle the edge.
    let budget = columns.saturating_sub(1);
    let mut out = String::new();
    let mut used = 0;
    for g in metrics::graphemes(text, DEFAULT_TAB_WIDTH) {
        if used + g.width > budget {
            break;
        }
        if g.is_tab() {
            out.extend(std::iter::repeat_n(' ', g.width as usize));
        } else {
            out.push_str(&visible(g.text));
        }
        used += g.width;
    }
    out.push('…');
    pad(&out, columns)
}
