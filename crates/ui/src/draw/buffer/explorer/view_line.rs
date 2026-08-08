//! One line of the file list: what is on it, and where it sits.
//!
//! Named for what it draws — a [`ViewLine`] — and not for a node, because a
//! heading has no node behind it. That is D69: a heading is what an
//! arrangement sits under, so it is the explorer's and never a tree's. One
//! function here per variant, so this file's shape is the enum's.
//!
//! The file list reports facts — this row is a directory, it is the last of
//! its siblings, its second ancestor was not. This file turns those into text
//! and colour: `▾`, `+4`, `M`. That `align` reports a gap and never says a gap
//! is drawn `╱` is the same division. See D65.
//!
//! Arranging is part of drawing a line, not a brick of its own. Where each
//! piece goes, and which of them survive a pane too narrow to hold them, is
//! one question with one answer, and it is asked here because this is where
//! the pieces are. A tree of commits would arrange its own rows: a graph
//! prefix, a subject, an author pinned right — the same shape, different
//! contents, and nothing shared but `line_index`, which is what counts columns
//! for everyone.
//!
//! There is no cached layout, so a resize needs no invalidation: a row is
//! arranged against the width it is being drawn into, every frame.

use ratatui::buffer::Buffer as Cells;
use ratatui::layout::Rect;
use ratatui::style::{Modifier, Style};

use file_types::{ChangeType, File, Stats};

use crate::render::cells;
use crate::theme::Theme;
use crate::view::buffer::explorer::ViewLine;

/// The one column that always separates the two sides.
const GAP: usize = 1;

/// The order pieces are dropped in when a row will not fit.
///
/// Named rather than written as numbers at each use, so that "stats go before
/// the guides" is a statement in one place instead of a comparison a reader
/// has to make between two distant literals.
pub mod priority {
    /// Where a moved file came from: useful, never essential.
    pub const MOVED: u8 = 0;
    /// The line counts.
    pub const STATS: u8 = 1;
    /// How many files a section holds.
    pub const COUNT: u8 = 2;
    /// The indent guides. Last to go, because losing them makes the tree
    /// unreadable rather than merely plainer.
    pub const GUIDES: u8 = 3;
}

/// One piece of a row: some text, how it looks, and whether it may go.
///
/// The style is resolved before arranging rather than after, so that nothing
/// below has to be told what the text *means* in order to decide whether it
/// survives.
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
    fn cut(&mut self, cells: usize) {
        let line = line_index::LineIndex::new(&self.text, 1);
        let end = line.cell_to_byte(line_index::CellCol(cells as u32));
        self.text.truncate(end.0 as usize);
    }
}

/// Draws one line: its prefix, what it says, and the letter at the edge.
///
/// `prefix` is whatever the shape put in front — indent guides and a fold
/// arrow in the nested one, nothing in the flat one.
pub fn draw(
    cells: &mut Cells,
    line: Rect,
    line_content: &ViewLine<'_>,
    prefix: Vec<Piece>,
    theme: &Theme,
    background: Style,
) {
    let (mut left, right) = match line_content {
        ViewLine::Heading { name, files, stats } => {
            heading(name, *files, *stats, theme, background)
        }
        ViewLine::Directory { name, .. } => directory(name, theme, background),
        ViewLine::File { name, file } => self::file(name, file, theme, background),
    };
    let mut pieces = prefix;
    pieces.append(&mut left);
    place(cells, line, pieces, right);
}

/// A heading: its name, how many files it holds, and their total.
fn heading(
    name: &str,
    files: usize,
    stats: Stats,
    theme: &Theme,
    background: Style,
) -> (Vec<Piece>, Vec<Piece>) {
    // Bold is applied here rather than stored in the theme: a heading is bold
    // in every theme, so it is structural. That is the same division `Code`
    // makes, where weight comes from the scope table and colour from the
    // theme.
    let mut left = vec![Piece::fixed(
        name,
        background
            .fg(theme.tree.heading)
            .add_modifier(Modifier::BOLD),
    )];
    let count = background.fg(theme.tree.count);
    if stats.is_empty() {
        left.push(Piece::droppable(
            format!(" ({files})"),
            count,
            priority::COUNT,
        ));
    } else {
        left.push(Piece::droppable(
            format!(" ({files} · "),
            count,
            priority::COUNT,
        ));
        push_stats(&mut left, stats, priority::COUNT, theme, background);
        left.push(Piece::droppable(")", count, priority::COUNT));
    }
    (left, Vec::new())
}

/// A directory: its name, and nothing at the right-hand edge.
///
/// Whether it is open is drawn in front of the name, by the arrangement that
/// knows what nests — see [`tree::prefix`](super::tree::prefix).
fn directory(name: &str, theme: &Theme, background: Style) -> (Vec<Piece>, Vec<Piece>) {
    (
        vec![Piece::fixed(name, background.fg(theme.tree.directory))],
        Vec::new(),
    )
}

/// A file: its name, where it came from, what it gained, and what happened.
fn file(name: &str, file: &File, theme: &Theme, background: Style) -> (Vec<Piece>, Vec<Piece>) {
    let mut left = vec![Piece::fixed(name, background.fg(theme.tree.name))];
    if let Some(previous) = file.previous_path() {
        left.push(Piece::droppable(
            format!(" ← {previous}"),
            background.fg(theme.tree.previous),
            priority::MOVED,
        ));
    }

    let mut right = Vec::new();
    // A file that gained and lost nothing says nothing, rather than `+0 -0` in
    // a column the eye is scanning.
    if let Some(stats) = file.get_stats().filter(|s| !s.is_empty()) {
        push_stats(&mut right, stats, priority::STATS, theme, background);
        // The space between the counts and the letter, which goes with them
        // rather than staying behind as a lone column.
        right.push(Piece::droppable(
            " ",
            background.fg(theme.tree.name),
            priority::STATS,
        ));
    }
    right.push(Piece::fixed(
        letter(file.get_change_type()),
        background
            .fg(theme.change.of(file.get_change_type()))
            .add_modifier(Modifier::BOLD),
    ));
    (left, right)
}

/// The `+4 -3` pair, with a side left out when it is zero.
///
/// A file that only gained lines says `+4`, not `+4 -0`: the zero is noise in
/// a column the eye is scanning.
fn push_stats(
    pieces: &mut Vec<Piece>,
    stats: Stats,
    priority: u8,
    theme: &Theme,
    background: Style,
) {
    if stats.added > 0 {
        pieces.push(Piece::droppable(
            format!("+{}", stats.added),
            background.fg(theme.change.gained),
            priority,
        ));
    }
    if stats.removed > 0 {
        let separator = if stats.added > 0 { " " } else { "" };
        pieces.push(Piece::droppable(
            format!("{separator}-{}", stats.removed),
            background.fg(theme.change.lost),
            priority,
        ));
    }
}

/// Git's letter for what happened.
///
/// Git's letters where a [`ChangeType`] has one. It has six variants and git
/// prints eight: a copy arrives as `Moved` and shows `R`, and a **type
/// change** as `Modified` and shows `M`. Both are deliberate — what a reviewer
/// does about either is read the new content, which is what those letters
/// already promise.
pub fn letter(change: ChangeType) -> &'static str {
    match change {
        ChangeType::Added => "A",
        ChangeType::Modified => "M",
        ChangeType::Deleted => "D",
        ChangeType::Moved => "R",
        ChangeType::Untracked => "??",
        ChangeType::Conflicted => "!",
    }
}

/// Puts the surviving pieces on the line, the right side against the edge.
///
/// Two rules, in this order. Whole pieces are dropped by
/// [`priority`](Piece::priority), lowest first and a whole level at a time —
/// so a count never loses its closing bracket while keeping its opening one.
/// Only when nothing is left to drop is a piece cut, and the widest is chosen,
/// because cutting the longest removes the most for the least loss.
fn place(cells: &mut Cells, line: Rect, mut left: Vec<Piece>, mut right: Vec<Piece>) {
    let width = line.width as usize;

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
        "a row of {} columns was placed in {width}",
        total(&left, &right)
    );

    // The gap takes every spare column, which is what pins the right side to
    // the edge at any width — and why a resize needs nothing invalidated.
    let spare = width.saturating_sub(sum(&left) + sum(&right));
    let gap = if left.is_empty() || right.is_empty() {
        0
    } else {
        spare.max(GAP)
    };

    let mut x = 0;
    for piece in &left {
        x = cells::write(cells, line, x, &piece.text, piece.style);
    }
    x += gap as u16;
    for piece in &right {
        x = cells::write(cells, line, x, &piece.text, piece.style);
    }
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

    /// What `place` would draw, as a string, without a terminal.
    fn shown(left: &[Piece], right: &[Piece], width: usize) -> String {
        let mut cells = Cells::empty(Rect::new(0, 0, width as u16, 1));
        let line = Rect::new(0, 0, width as u16, 1);
        place(&mut cells, line, left.to_vec(), right.to_vec());
        (0..width as u16)
            .map(|x| cells[(x, 0)].symbol().to_owned())
            .collect::<String>()
            .trim_end()
            .to_owned()
    }

    #[test]
    fn a_wide_pane_pushes_the_two_sides_apart() {
        assert_eq!(
            shown(&[fixed("a.rs")], &[fixed("M")], 20),
            "a.rs".to_owned() + &" ".repeat(15) + "M"
        );
    }

    #[test]
    fn the_lowest_priority_goes_first() {
        let left = [fixed("a.rs"), droppable(" ← old.rs", priority::MOVED)];
        let right = [droppable("+4", priority::STATS), fixed("M")];
        assert_eq!(shown(&left, &right, 20), "a.rs ← old.rs    +4M");
        assert_eq!(shown(&left, &right, 12), "a.rs     +4M");
        assert_eq!(shown(&left, &right, 6), "a.rs M");
    }

    #[test]
    fn a_whole_priority_level_goes_at_once() {
        // The failure this prevents: `(3 · ` kept while `)` is dropped,
        // leaving a bracket that never closes.
        let left = [
            fixed("Changes"),
            droppable(" (3 · ", priority::COUNT),
            droppable("+9", priority::COUNT),
            droppable(")", priority::COUNT),
        ];
        assert_eq!(shown(&left, &[], 10), "Changes");
    }

    #[test]
    fn what_cannot_be_dropped_is_cut_and_says_so() {
        assert_eq!(
            shown(&[fixed("a-very-long-file-name.rs")], &[fixed("M")], 12),
            "a-very-lo… M"
        );
    }

    #[test]
    fn the_widest_is_the_one_that_is_cut() {
        // Cutting the indent guides instead of the name would save the same
        // columns and cost the reader the thing they were looking for.
        let left = [fixed("│ │ "), fixed("a-long-name.rs")];
        assert_eq!(shown(&left, &[], 10), "│ │ a-lon…");
    }

    #[test]
    fn a_row_never_comes_out_wider_than_the_pane() {
        let rows: [(&[Piece], &[Piece]); 4] = [
            (&[fixed("a.rs")], &[fixed("M")]),
            (
                &[
                    fixed("│ │ "),
                    fixed("a-long-name.rs"),
                    droppable(" ← was.rs", priority::MOVED),
                ],
                &[droppable("+12 -3", priority::STATS), fixed("M")],
            ),
            (&[fixed("ファイル.txt")], &[fixed("??")]),
            (&[], &[]),
        ];
        for width in 1..30usize {
            for (left, right) in rows {
                let mut fitted_left = left.to_vec();
                let mut fitted_right = right.to_vec();
                // The same two rules `place` applies, checked on their own so
                // the assertion is about columns rather than about the grid,
                // which clips and would hide the failure.
                while total(&fitted_left, &fitted_right) > width {
                    let Some(level) = lowest_priority(&fitted_left, &fitted_right) else {
                        break;
                    };
                    fitted_left.retain(|p| p.priority != Some(level));
                    fitted_right.retain(|p| p.priority != Some(level));
                }
                while total(&fitted_left, &fitted_right) > width
                    && !(fitted_left.is_empty() && fitted_right.is_empty())
                {
                    let over = total(&fitted_left, &fitted_right) - width;
                    if !truncate_widest(&mut fitted_left, &mut fitted_right, over) {
                        break;
                    }
                }
                let drawn = total(&fitted_left, &fitted_right);
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

    #[test]
    fn every_change_has_a_letter_of_its_own() {
        // A letter shared by two changes would make one of them unreadable in
        // a column the eye is scanning.
        let changes = [
            ChangeType::Added,
            ChangeType::Modified,
            ChangeType::Deleted,
            ChangeType::Moved,
            ChangeType::Untracked,
            ChangeType::Conflicted,
        ];
        let mut seen: Vec<&str> = changes.iter().map(|&c| letter(c)).collect();
        seen.sort_unstable();
        let count = seen.len();
        seen.dedup();
        assert_eq!(seen.len(), count, "two changes share a letter");
    }
}
