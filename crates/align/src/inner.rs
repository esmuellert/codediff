//! Converts the engine's UTF-16 character ranges into per-line byte spans.

use diff_types::CharRange;
use line_index::{DEFAULT_TAB_WIDTH, LineIndex, Utf16Col};

/// A run of changed characters within one line.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Span {
    /// The line it sits on, numbered from 1.
    pub line: u32,
    /// Byte offsets into that line, half-open.
    pub bytes: std::ops::Range<u32>,
}

/// Splits a range into non-empty spans, one per affected line.
pub fn spans<S: AsRef<str>>(range: &CharRange, lines: &[S]) -> Vec<Span> {
    spans_with_tab_width(range, lines, DEFAULT_TAB_WIDTH)
}

pub fn spans_with_tab_width<S: AsRef<str>>(
    range: &CharRange,
    lines: &[S],
    tab_width: u8,
) -> Vec<Span> {
    (range.start_line..=range.end_line)
        .filter_map(|line| span_on(range, line, lines, tab_width))
        .collect()
}

/// The part of `range` that falls on one line, if any.
///
/// Split out so a caller asking about a single line does not pay to expand
/// every other line the range touches.
pub fn span_on<S: AsRef<str>>(
    range: &CharRange,
    line: u32,
    lines: &[S],
    tab_width: u8,
) -> Option<Span> {
    if line < range.start_line || line > range.end_line {
        return None;
    }
    let text = lines.get(line.checked_sub(1)? as usize)?.as_ref();
    let index = LineIndex::new(text, tab_width);

    // The first line starts where the range does, later lines at column 0; the
    // last line stops where the range does, earlier lines at their end.
    let from = if line == range.start_line {
        Utf16Col::from_engine(range.start_col)
    } else {
        Utf16Col::ZERO
    };
    let to = if line == range.end_line {
        Utf16Col::from_engine(range.end_col)
    } else {
        index.utf16_len()
    };

    let bytes = index.utf16_range_to_bytes(from..to);
    if bytes.start >= bytes.end {
        return None;
    }
    Some(Span {
        line,
        bytes: bytes.start.get()..bytes.end.get(),
    })
}
