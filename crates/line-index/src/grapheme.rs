//! Walking a line, cluster by cluster.
//!
//! The counterpart to [`LineIndex`](crate::LineIndex), which *queries* a
//! line. Drawing needs only a forward walk and no index, so the two are kept
//! apart: nothing here allocates.

use unicode_segmentation::UnicodeSegmentation;

use crate::coord::{ByteOff, CellCol, Utf16Col};
use crate::width::{grapheme_width, tab_advance};

/// One grapheme cluster, with its position in every coordinate system.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Grapheme<'a> {
    /// The cluster itself, borrowed from the line. A tab is `"\t"`; expanding
    /// it to spaces is the renderer's job, using [`Grapheme::width`].
    pub text: &'a str,
    pub byte: ByteOff,
    pub utf16: Utf16Col,
    pub cell: CellCol,
    /// Terminal columns occupied. Two for wide characters, and for a tab the
    /// distance to the next tab stop.
    pub width: u32,
}

impl Grapheme<'_> {
    pub fn is_tab(&self) -> bool {
        self.text == "\t"
    }

    /// The half-open cell range this cluster covers.
    pub fn cells(&self) -> std::ops::Range<u32> {
        self.cell.get()..self.cell.get() + self.width
    }
}

/// A grapheme boundary, in all three coordinate systems at once.
///
/// Doubles as an entry in [`LineIndex`](crate::LineIndex)' index and as a
/// place to resume a walk from, which is how scrolling right avoids rewalking
/// the line from column zero.
#[derive(Debug, Clone, Copy)]
pub(crate) struct Position {
    pub(crate) byte: u32,
    pub(crate) utf16: u32,
    pub(crate) cell: u32,
}

impl Position {
    pub(crate) const ORIGIN: Self = Self {
        byte: 0,
        utf16: 0,
        cell: 0,
    };
}

/// Grapheme clusters of a line, with their position in every coordinate
/// system, without building an index.
///
/// Drawing a line needs only a forward walk, so paying for a
/// [`LineIndex`](crate::LineIndex) table that nothing queries is waste —
/// and a renderer rebuilds its visible lines on every frame. Use this to draw,
/// and `LineIndex` to answer questions about positions.
pub fn graphemes(text: &str, tab_width: u8) -> impl Iterator<Item = Grapheme<'_>> {
    graphemes_from(text, tab_width, Position::ORIGIN)
}

/// Walks clusters from an already-known position part-way into the line.
pub(crate) fn graphemes_from(
    text: &str,
    tab_width: u8,
    from: Position,
) -> impl Iterator<Item = Grapheme<'_>> {
    let (mut byte, mut utf16, mut cell) = (from.byte, from.utf16, from.cell);
    text[from.byte as usize..]
        .graphemes(true)
        .map(move |cluster| {
            let width = if cluster == "\t" {
                tab_advance(CellCol(cell), tab_width)
            } else {
                grapheme_width(cluster)
            };
            let item = Grapheme {
                text: cluster,
                byte: ByteOff(byte),
                utf16: Utf16Col(utf16),
                cell: CellCol(cell),
                width,
            };
            byte = byte.saturating_add(len32(cluster));
            utf16 = utf16.saturating_add(utf16_len(cluster));
            cell = cell.saturating_add(width);
            item
        })
}

/// Byte length as `u32`.
///
/// Every coordinate in this crate is a `u32`, which caps a single line at 4
/// GiB. Nothing that reaches a code reviewer comes close, and saturating keeps
/// positions monotone — the property binary search depends on — where wrapping
/// would not.
pub(crate) fn len32(text: &str) -> u32 {
    u32::try_from(text.len()).unwrap_or(u32::MAX)
}

pub(crate) fn utf16_len(text: &str) -> u32 {
    text.chars()
        .map(|c| c.len_utf16() as u32)
        .fold(0u32, u32::saturating_add)
}
