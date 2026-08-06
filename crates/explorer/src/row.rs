//! One visible line, and the pieces it is made of.
//!
//! A row is text and a classification, never a colour and never a cell. What
//! a status letter looks like is a theme's business; what makes it a status
//! letter is this crate's.
//!
//! The pieces are split left and right because the row is drawn from both
//! ends: the name grows rightwards from the indent, and the status letter is
//! pinned to the edge so the eye can run down the column. When there is not
//! room for both, [`Region::priority`] says what goes first.

use file_types::ChangeType;

use crate::node::NodeId;

/// What one piece of a row is, so a theme can colour it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RegionType {
    /// The indent guides, `│ ` and `├ `.
    Marker,
    /// Whether a foldable row is open or shut.
    Fold,
    /// A section heading.
    Heading,
    /// A directory's name.
    Directory,
    /// A file's name.
    Name,
    /// Where a moved file came from.
    Moved,
    /// How many files a section holds.
    Count,
    Added,
    Removed,
    /// Git's letter for what happened, and what happened.
    ///
    /// The change travels with the region so a theme can colour a deletion
    /// differently from an addition. What a letter *means* is this crate's
    /// answer; what it looks like is the theme's — the same split as
    /// `syntax::Group` against `theme::Code`, and the reason neither can
    /// silently take on the other's job.
    Status(ChangeType),
    /// The space between the counts and the letter.
    Spacer,
}

/// One piece of a row.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Region {
    pub text: String,
    pub region_type: RegionType,
    /// What is dropped first when the pane is too narrow, lowest first.
    ///
    /// `None` is never dropped. The name has no priority for that reason: a
    /// row with no name says nothing at all, so it is cut with an ellipsis
    /// instead.
    pub priority: Option<u8>,
}

impl Region {
    /// A piece that survives any width.
    pub fn fixed(text: impl Into<String>, region_type: RegionType) -> Self {
        Self {
            text: text.into(),
            region_type,
            priority: None,
        }
    }

    /// A piece that is dropped when the row will not fit.
    pub fn droppable(text: impl Into<String>, region_type: RegionType, priority: u8) -> Self {
        Self {
            text: text.into(),
            region_type,
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

/// One visible line of the explorer.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Row {
    /// Which node this row shows, so a key press has something to act on.
    pub node: NodeId,
    /// Pieces written from the left edge rightwards.
    pub left: Vec<Region>,
    /// Pieces pinned to the right edge.
    pub right: Vec<Region>,
}

impl Row {
    /// The row as one string, at its narrowest — for tests and for comparing
    /// against a capture of the plugin.
    ///
    /// The two sides are joined by the one space that always separates them.
    /// The real gap depends on the width of the pane, which a row does not
    /// know, so this is the only spelling of it that is true at every width.
    pub fn text(&self) -> String {
        let mut out = String::new();
        for region in &self.left {
            out.push_str(&region.text);
        }
        if !self.left.is_empty() && !self.right.is_empty() {
            out.push(' ');
        }
        for region in &self.right {
            out.push_str(&region.text);
        }
        out
    }
}

/// The order regions are dropped in when a row will not fit.
///
/// Named rather than written as numbers at each call, so that "stats go before
/// the directory" is a statement in one place instead of a comparison a reader
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
    pub const MARKER: u8 = 3;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn width_counts_characters_and_not_bytes() {
        // A row of accented names would otherwise be measured at three times
        // its length and truncated to nothing.
        let region = Region::fixed("ünïcodé", RegionType::Name);
        assert_eq!(region.width(), 7);
        assert_eq!(region.text.len(), 10, "and the bytes are not the width");
    }

    #[test]
    fn a_wide_character_is_two_columns() {
        // The failure this prevents: a Japanese file name measured at one
        // column per character, pushing the status letter off the pane.
        let region = Region::fixed("ファイル.txt", RegionType::Name);
        assert_eq!(region.width(), 12);
    }

    #[test]
    fn a_cut_never_lands_inside_a_wide_character() {
        let mut region = Region::fixed("ファイル", RegionType::Name);
        region.cut(3);
        assert_eq!(region.text, "フ", "the third column is half a glyph");
        assert_eq!(region.width(), 2, "so the row gives up a column instead");
        region.cut(4);
        assert_eq!(
            region.text, "フ",
            "and a cut wider than the text is a no-op"
        );
    }
}
