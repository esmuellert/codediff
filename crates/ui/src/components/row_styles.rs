//! What every row of a diff has in common: the styles it wears, the width its
//! numbers need, and the part of a selection that falls on it.

use align::{DiffVersion, ViewLineType};
use ratatui::style::Style;

use crate::theme::Theme;

/// The three styles one row wears.
///
/// Priority: change > moved > cursor > plain. A change outranks the cursor
/// line because losing sight of which lines differ is worse than losing sight
/// of where the cursor is, and the line number still says where it is.
pub fn row_styles(
    theme: &Theme,
    kind: ViewLineType,
    diff_version: DiffVersion,
    moved: bool,
    is_cursor: bool,
) -> (Style, Style, Style) {
    let role = if kind != ViewLineType::Unchanged {
        match diff_version {
            DiffVersion::Original => theme.deleted,
            DiffVersion::Modified => theme.inserted,
        }
    } else if moved {
        theme.moved
    } else if is_cursor {
        theme.cursor_line
    } else {
        Style::new()
    };

    let unchanged = theme.normal.patch(role);
    let changed = unchanged.patch(match (kind, diff_version) {
        (ViewLineType::Unchanged, _) => Style::new(),
        (_, DiffVersion::Original) => theme.deleted_text,
        (_, DiffVersion::Modified) => theme.inserted_text,
    });
    let numbers = unchanged.patch(if is_cursor {
        theme.line_number_current
    } else {
        theme.line_number
    });

    (unchanged, changed, numbers)
}

/// The part of a selection that falls on one line, in cells.
///
/// `None` when the selection does not reach this line at all.
pub fn clip_to_line(
    selection: Option<&crate::view::selection::Selection>,
    line: u32,
) -> Option<std::ops::Range<u32>> {
    let selection = selection?;
    let start = selection.start_pos();
    let end = selection.end_pos();
    if line < start.line || line > end.line {
        return None;
    }
    let from = if line == start.line { start.col } else { 0 };
    let to = if line == end.line { end.col.saturating_add(1) } else { u32::MAX };
    Some(from..to)
}

/// How wide the gutter has to be for a file with this many lines.
///
/// One space after the widest number, and never narrower than three digits so
/// the text column does not jump as a file scrolls past 99.
pub fn gutter_width(lines: u32) -> u16 {
    let digits = lines.max(1).ilog10() + 1;
    (digits as u16).max(3) + 1
}

#[cfg(test)]
mod tests {
    use super::gutter_width;

    #[test]
    fn a_short_file_still_gets_three_digits_and_a_space() {
        assert_eq!(gutter_width(1), 4);
        assert_eq!(gutter_width(99), 4);
        assert_eq!(gutter_width(999), 4);
    }

    #[test]
    fn a_long_file_widens_by_one_per_digit() {
        assert_eq!(gutter_width(1_000), 5);
        assert_eq!(gutter_width(10_000), 6);
    }
}
