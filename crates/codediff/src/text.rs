//! Turning file content into something safe to print.
//!
//! Shared by every `debug` subcommand. The substitution itself lives in
//! `line-index`, beside the code that measures those characters as one column,
//! so the two cannot disagree; what is here is the padding and fitting that
//! only a text-mode command needs.

use line_index::{DEFAULT_TAB_WIDTH, LineIndex};

pub use line_index::visible;

/// Tabs replaced by the spaces they expand to, and controls by their picture.
///
/// A raw tab would use the *terminal's* tab stops rather than the ones we
/// measured with, so the two would disagree about where anything sits.
pub fn expand(line: &LineIndex<'_>) -> String {
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
    expand(&LineIndex::new(text, DEFAULT_TAB_WIDTH))
}

/// Terminal columns the text occupies.
pub fn display_width(text: &str) -> u32 {
    LineIndex::new(text, DEFAULT_TAB_WIDTH).width().get()
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
    for g in line_index::graphemes(text, DEFAULT_TAB_WIDTH) {
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
