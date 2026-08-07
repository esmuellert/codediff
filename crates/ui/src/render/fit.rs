//! Fitting a row of pieces into the width it has.
//!
//! A brick: it is handed text, styles and a width, and knows nothing about
//! trees, files or diffs. That is what lets "does a narrow pane keep the file
//! name" be asked of a list of strings rather than of a screenshot.
//!
//! Only the file list calls this today. It is separate from
//! [`list`](super::list) because the rule is not the list's: the status line
//! shortens a path by the same one, written a second time and by hand — see
//! B9, which this is what fixes.
//!
//! Two rules, in this order. Whole pieces are dropped by
//! [`priority`](Piece::priority), lowest first and a whole level at a time —
//! so a count never loses its closing bracket while keeping its opening one.
//! Only when nothing is left to drop is a piece cut, and the widest is chosen,
//! because cutting the longest removes the most for the least loss.

use ratatui::style::Style;

/// The one column that always separates the two sides.
const GAP: usize = 1;

/// One piece of a row: some text, how it looks, and whether it may go.
///
/// The style is resolved before fitting rather than after, so that nothing
/// here has to be told what the text *means* in order to decide whether it
/// survives. That is what makes this reusable by anything that draws a row of
/// parts — a file list, or a status line.
#[derive(Debug, Clone, PartialEq)]
pub struct Piece {
    pub text: String,
    pub style: Style,
    /// What is dropped first when the row is too wide, lowest first.
    ///
    /// `None` is never dropped. A file's name has none for that reason: a row
    /// with no name says nothing at all, so it is cut with an ellipsis
    /// instead.
    pub priority: Option<u8>,
}

impl Piece {
    /// A piece that survives any width.
    pub fn fixed(text: impl Into<String>, style: Style) -> Self {
        Self {
            text: text.into(),
            style,
            priority: None,
        }
    }

    /// A piece that is dropped when the row will not fit.
    pub fn droppable(text: impl Into<String>, style: Style, priority: u8) -> Self {
        Self {
            text: text.into(),
            style,
            priority: Some(priority),
        }
    }

    /// Columns this piece takes on screen.
    ///
    /// Cells, not characters. A Japanese file name is two columns per
    /// character, and measuring it as one each puts the status letter past the
    /// right edge of the pane, where it is not drawn at all.
    pub fn width(&self) -> usize {
        line_index::LineIndex::new(&self.text, 1).width().0 as usize
    }

    /// Cuts this piece to `cells` columns.
    ///
    /// Never through the middle of a character: a wide one that will not fit
    /// is dropped, leaving the row a column short rather than a broken glyph.
    pub fn cut(&mut self, cells: usize) {
        let line = line_index::LineIndex::new(&self.text, 1);
        let end = line.cell_to_byte(line_index::CellCol(cells as u32));
        self.text.truncate(end.0 as usize);
    }
}

/// What a row will actually show at a given width.
#[derive(Debug, Clone, PartialEq)]
pub struct Fitted {
    pub left: Vec<Piece>,
    pub right: Vec<Piece>,
    /// Columns between the two sides. At least [`GAP`], and everything spare
    /// when there is room.
    pub gap: usize,
}

/// Chooses what survives at `width` columns.
pub fn fit(left: &[Piece], right: &[Piece], width: usize) -> Fitted {
    let mut left = left.to_vec();
    let mut right = right.to_vec();

    while total(&left, &right) > width {
        let Some(level) = lowest_priority(&left, &right) else {
            break;
        };
        left.retain(|piece| piece.priority != Some(level));
        right.retain(|piece| piece.priority != Some(level));
    }

    // A loop, not one pass: dropping the widest piece can leave the row still
    // too wide, and returning without re-checking broke the promise this
    // function's name makes. It was invisible only because the cell writer
    // clips.
    while total(&left, &right) > width && !(left.is_empty() && right.is_empty()) {
        let over = total(&left, &right) - width;
        if !truncate_widest(&mut left, &mut right, over) {
            break;
        }
    }
    debug_assert!(
        total(&left, &right) <= width || (left.is_empty() && right.is_empty()),
        "a row of {} columns was fitted into {width}",
        total(&left, &right)
    );

    let spare = width.saturating_sub(sum(&left) + sum(&right));
    // No gap when there is nothing on the other side of it: a heading would
    // otherwise be followed by a column of trailing space that widens the row
    // past the pane.
    let gap = if left.is_empty() || right.is_empty() {
        0
    } else {
        spare.max(GAP)
    };
    Fitted { left, right, gap }
}

fn sum(pieces: &[Piece]) -> usize {
    pieces.iter().map(Piece::width).sum()
}

/// What the row needs, including the space between the sides.
fn total(left: &[Piece], right: &[Piece]) -> usize {
    let gap = if left.is_empty() || right.is_empty() {
        0
    } else {
        GAP
    };
    sum(left) + gap + sum(right)
}

fn lowest_priority(left: &[Piece], right: &[Piece]) -> Option<u8> {
    left.iter()
        .chain(right)
        .filter_map(|piece| piece.priority)
        .min()
}

/// Cuts `over` columns from the widest piece, ending it with an ellipsis.
///
/// The ellipsis costs a column of its own, so a piece cut to nothing is
/// dropped rather than left as a lone `…` saying nothing about what was there.
///
/// Returns whether anything changed, so the caller's loop cannot spin on a row
/// it can no longer make narrower.
fn truncate_widest(left: &mut Vec<Piece>, right: &mut Vec<Piece>, over: usize) -> bool {
    let widest = left
        .iter_mut()
        .chain(right.iter_mut())
        .max_by_key(|piece| piece.width());
    let Some(piece) = widest else {
        return false;
    };
    if piece.width() == 0 {
        return false;
    }
    let keep = piece.width().saturating_sub(over + 1);
    if keep == 0 {
        piece.text.clear();
        left.retain(|piece| !piece.text.is_empty());
        right.retain(|piece| !piece.text.is_empty());
        return true;
    }
    // Cut in cells, not characters: a wide glyph is two columns, and cutting
    // by character count would leave the row wider than the pane.
    piece.cut(keep);
    piece.text.push('…');
    true
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fixed(text: &str) -> Piece {
        Piece::fixed(text, Style::default())
    }

    fn droppable(text: &str, priority: u8) -> Piece {
        Piece::droppable(text, Style::default(), priority)
    }

    fn shown(fitted: &Fitted) -> String {
        let mut out = String::new();
        for piece in &fitted.left {
            out.push_str(&piece.text);
        }
        out.push_str(&" ".repeat(fitted.gap));
        for piece in &fitted.right {
            out.push_str(&piece.text);
        }
        out
    }

    #[test]
    fn a_wide_pane_pushes_the_two_sides_apart() {
        let fitted = fit(&[fixed("a.rs")], &[fixed("M")], 20);
        assert_eq!(shown(&fitted), "a.rs               M");
        assert_eq!(shown(&fitted).chars().count(), 20);
    }

    #[test]
    fn the_lowest_priority_goes_first() {
        let left = [fixed("a.rs"), droppable(" ← old.rs", 0)];
        let right = [droppable("+4", 1), fixed("M")];
        assert_eq!(shown(&fit(&left, &right, 20)), "a.rs ← old.rs    +4M");
        assert_eq!(shown(&fit(&left, &right, 12)), "a.rs     +4M");
        assert_eq!(shown(&fit(&left, &right, 6)), "a.rs M");
    }

    #[test]
    fn a_whole_priority_level_goes_at_once() {
        // The failure this prevents: `(3 · ` kept while `)` is dropped,
        // leaving a bracket that never closes.
        let left = [
            fixed("Changes"),
            droppable(" (3 · ", 2),
            droppable("+9", 2),
            droppable(")", 2),
        ];
        let fitted = fit(&left, &[], 10);
        assert_eq!(shown(&fitted), "Changes");
    }

    #[test]
    fn what_cannot_be_dropped_is_cut_and_says_so() {
        let left = [fixed("a-very-long-file-name.rs")];
        let fitted = fit(&left, &[fixed("M")], 12);
        assert_eq!(shown(&fitted), "a-very-lo… M");
        assert_eq!(fitted.left[0].width(), 10, "the ellipsis is one of them");
    }

    #[test]
    fn the_widest_is_the_one_that_is_cut() {
        // Cutting the indent guides instead of the name would save the same
        // columns and cost the reader the thing they were looking for.
        let left = [fixed("│ │ "), fixed("a-long-name.rs")];
        let fitted = fit(&left, &[], 10);
        assert_eq!(shown(&fitted), "│ │ a-lon…");
    }

    #[test]
    fn a_row_never_comes_out_wider_than_the_pane() {
        // The assertion used to be `<= width.max(2)`, which at widths 0 and 1
        // permitted two columns and so asserted nothing at all. It really only
        // checked that nothing panicked, which its name half admitted.
        let rows: [(&[Piece], &[Piece]); 4] = [
            (&[fixed("a.rs")], &[fixed("M")]),
            (
                &[
                    fixed("│ │ "),
                    fixed("a-long-name.rs"),
                    droppable(" ← was.rs", 0),
                ],
                &[droppable("+12 -3", 1), fixed("M")],
            ),
            (&[fixed("ファイル.txt")], &[fixed("??")]),
            (&[], &[]),
        ];
        for width in 0..30 {
            for (left, right) in rows {
                let fitted = fit(left, right, width);
                let drawn = shown(&fitted).chars().count();
                assert!(drawn <= width, "{drawn} columns drawn into {width}");
            }
        }
    }

    #[test]
    fn width_counts_columns_and_not_bytes() {
        // A row of accented names would otherwise be measured at three times
        // its length and truncated to nothing.
        let piece = fixed("ünïcodé");
        assert_eq!(piece.width(), 7);
        assert_eq!(piece.text.len(), 10, "and the bytes are not the width");
    }

    #[test]
    fn a_wide_character_is_two_columns() {
        // The failure this prevents: a Japanese file name measured at one
        // column per character, pushing the status letter off the pane.
        assert_eq!(fixed("ファイル.txt").width(), 12);
    }

    #[test]
    fn a_cut_never_lands_inside_a_wide_character() {
        let mut piece = fixed("ファイル");
        piece.cut(3);
        assert_eq!(piece.text, "フ", "the third column is half a glyph");
        assert_eq!(piece.width(), 2, "so the row gives up a column instead");
        piece.cut(4);
        assert_eq!(piece.text, "フ", "and a cut wider than the text is a no-op");
    }
}
