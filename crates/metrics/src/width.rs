//! How much horizontal space a piece of text occupies on a terminal.

use unicode_width::UnicodeWidthStr;

use crate::coord::CellCol;

/// Terminal columns occupied by one grapheme cluster.
///
/// Combining marks contribute zero, so `e` followed by U+0301 measures one.
/// East Asian wide and fullwidth characters measure two.
///
/// Control characters other than tab are treated as one column: they are
/// rendered as a placeholder rather than being allowed to move the cursor.
pub fn grapheme_width(grapheme: &str) -> u32 {
    debug_assert!(!grapheme.is_empty(), "a grapheme cluster is never empty");

    if grapheme == "\t" {
        // Meaningless without a starting column; callers use `tab_advance`.
        return 1;
    }
    match grapheme.width() as u32 {
        // Unicode assigns zero width to control characters, but a terminal
        // still has to put something in that column.
        0 if !starts_with_combining(grapheme) => 1,
        width => width,
    }
}

/// Columns a tab occupies when it begins at `from`.
///
/// A tab advances to the next multiple of `tab_width`, so its width depends on
/// everything before it: the same tab is four columns wide at column 0 and one
/// column wide at column 3.
pub fn tab_advance(from: CellCol, tab_width: u8) -> u32 {
    let tab_width = u32::from(tab_width.max(1));
    tab_width - (from.get() % tab_width)
}

fn starts_with_combining(grapheme: &str) -> bool {
    grapheme
        .chars()
        .next()
        .is_some_and(|c| unicode_width::UnicodeWidthChar::width(c) == Some(0))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ascii_is_one_column() {
        assert_eq!(grapheme_width("a"), 1);
        assert_eq!(grapheme_width(" "), 1);
    }

    #[test]
    fn east_asian_characters_are_two_columns() {
        assert_eq!(grapheme_width("日"), 2);
        assert_eq!(grapheme_width("本"), 2);
    }

    #[test]
    fn a_combining_mark_does_not_widen_its_base() {
        // "e" followed by COMBINING ACUTE ACCENT is one grapheme, one column.
        assert_eq!(grapheme_width("e\u{0301}"), 1);
    }

    #[test]
    fn a_control_character_still_occupies_its_column() {
        assert_eq!(grapheme_width("\u{0}"), 1);
    }

    #[test]
    fn tabs_advance_to_the_next_stop() {
        assert_eq!(tab_advance(CellCol(0), 4), 4);
        assert_eq!(tab_advance(CellCol(1), 4), 3);
        assert_eq!(tab_advance(CellCol(3), 4), 1);
        assert_eq!(tab_advance(CellCol(4), 4), 4);
    }

    #[test]
    fn a_zero_tab_width_does_not_divide_by_zero() {
        assert_eq!(tab_advance(CellCol(0), 0), 1);
    }
}
