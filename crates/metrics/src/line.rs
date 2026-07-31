//! Positional queries about a single line of text.
//!
//! A [`LineMetrics`] indexes one line once, then answers conversions between
//! byte offsets, UTF-16 columns and terminal cells by binary search. Walking a
//! line rather than querying it lives in [`crate::grapheme`], and needs no
//! index.

use unicode_segmentation::UnicodeSegmentation;

use crate::coord::{ByteOff, CellCol, CharIdx, Utf16Col};
use crate::grapheme::{Grapheme, Position, graphemes_from, len32, utf16_len};
use crate::width::{grapheme_width, tab_advance};

#[derive(Debug, Clone)]
enum Index {
    /// Printable ASCII with no tabs: byte, char, UTF-16 and cell are all the
    /// same number, so no table is needed. This is the common case by a wide
    /// margin, and it is also why confusing the coordinates goes unnoticed.
    Trivial { len: u32 },
    /// One entry per grapheme boundary, plus a terminal entry, so that every
    /// query is a binary search.
    Mapped(Vec<Position>),
}

/// Measurements for one line of text.
#[derive(Debug, Clone)]
pub struct LineMetrics<'a> {
    text: &'a str,
    tab_width: u8,
    index: Index,
}

impl<'a> LineMetrics<'a> {
    /// Indexes `text`, which must be a single line without its terminator.
    ///
    /// Building the index costs an allocation on any line that is not plain
    /// ASCII. Code that only walks a line to draw it should call [`graphemes`]
    /// instead and leave this for positional queries.
    pub fn new(text: &'a str, tab_width: u8) -> Self {
        let index = if is_trivial(text) {
            Index::Trivial { len: len32(text) }
        } else {
            Index::Mapped(build_positions(text, tab_width))
        };
        Self {
            text,
            tab_width,
            index,
        }
    }

    pub fn text(&self) -> &'a str {
        self.text
    }

    pub fn tab_width(&self) -> u8 {
        self.tab_width
    }

    /// Total terminal columns occupied.
    pub fn width(&self) -> CellCol {
        CellCol(self.last().cell)
    }

    pub fn byte_len(&self) -> ByteOff {
        ByteOff(self.last().byte)
    }

    pub fn utf16_len(&self) -> Utf16Col {
        Utf16Col(self.last().utf16)
    }

    /// Byte offset of a UTF-16 column.
    ///
    /// Columns past the end clamp to the end of the line. A column landing
    /// inside a character — the second half of a surrogate pair — clamps to
    /// that character's start, so a highlight covers the whole character
    /// rather than half of one.
    pub fn utf16_to_byte(&self, col: Utf16Col) -> ByteOff {
        match &self.index {
            Index::Trivial { len } => ByteOff(col.get().min(*len)),
            Index::Mapped(stops) => {
                let target = col.get();
                let i = partition(stops, |s| s.utf16 <= target);
                let stop = stops[i];
                if stop.utf16 == target {
                    return ByteOff(stop.byte);
                }
                // Inside a cluster: walk its characters to the exact byte.
                let mut byte = stop.byte;
                let mut utf16 = stop.utf16;
                for ch in self.text[stop.byte as usize..].chars() {
                    if utf16 >= target {
                        break;
                    }
                    let next = utf16 + ch.len_utf16() as u32;
                    if next > target {
                        break; // mid-character; clamp to its start
                    }
                    byte += ch.len_utf8() as u32;
                    utf16 = next;
                }
                ByteOff(byte.min(self.last().byte))
            }
        }
    }

    /// Byte offset of a UTF-16 column, rounding a column that lands inside a
    /// character **up** to that character's end.
    ///
    /// The counterpart to [`utf16_to_byte`](Self::utf16_to_byte), which rounds
    /// down. Which one is correct depends on whether the column opens or closes
    /// a range; [`utf16_range_to_bytes`](Self::utf16_range_to_bytes) picks.
    pub fn utf16_to_byte_ceil(&self, col: Utf16Col) -> ByteOff {
        let floor = self.utf16_to_byte(col);
        if self.byte_to_utf16(floor) >= col {
            return floor;
        }
        match self.text[floor.get() as usize..].chars().next() {
            Some(ch) => ByteOff(floor.get().saturating_add(ch.len_utf8() as u32)),
            None => floor,
        }
    }

    /// Byte range covering a half-open span of UTF-16 columns.
    ///
    /// **This is the conversion the diff engine's inner-change spans need.**
    /// The engine compares individual UTF-16 code units, so it can report a
    /// span that begins or ends *inside* a character: `😀` and `🨀` differ only
    /// in their high surrogate, and the engine reports a one-unit change.
    /// Rounding both ends down would collapse that to an empty byte range and
    /// the change would be highlighted nowhere at all, so the start rounds down
    /// and the exclusive end rounds up. A partly covered character is covered
    /// whole.
    ///
    /// An empty span stays empty — a caret between two characters marks no
    /// text — so a caller can still distinguish "nothing changed here".
    ///
    /// Rounding is to whole *characters*. A caller that needs to highlight
    /// whole grapheme clusters, so that a combining mark travels with its base,
    /// can widen the result using [`graphemes`].
    pub fn utf16_range_to_bytes(
        &self,
        cols: std::ops::Range<Utf16Col>,
    ) -> std::ops::Range<ByteOff> {
        let start = self.utf16_to_byte(cols.start);
        if cols.start >= cols.end {
            return start..start;
        }
        let end = self.utf16_to_byte_ceil(cols.end);
        start..end.max(start)
    }

    /// UTF-16 column of a byte offset. Offsets inside a character clamp to its
    /// start.
    pub fn byte_to_utf16(&self, off: ByteOff) -> Utf16Col {
        match &self.index {
            Index::Trivial { len } => Utf16Col(off.get().min(*len)),
            Index::Mapped(stops) => {
                let target = off.get();
                let i = partition(stops, |s| s.byte <= target);
                let stop = stops[i];
                let mut byte = stop.byte;
                let mut utf16 = stop.utf16;
                for ch in self.text[stop.byte as usize..].chars() {
                    if byte >= target {
                        break;
                    }
                    let next = byte + ch.len_utf8() as u32;
                    if next > target {
                        break;
                    }
                    byte = next;
                    utf16 += ch.len_utf16() as u32;
                }
                Utf16Col(utf16)
            }
        }
    }

    /// The cell a byte offset begins at.
    pub fn byte_to_cell(&self, off: ByteOff) -> CellCol {
        match &self.index {
            Index::Trivial { len } => CellCol(off.get().min(*len)),
            Index::Mapped(stops) => {
                let target = off.get();
                let i = partition(stops, |s| s.byte <= target);
                CellCol(stops[i].cell)
            }
        }
    }

    /// The byte offset drawn at a cell.
    ///
    /// A cell inside a wide character clamps to that character's start, since
    /// there is no byte that begins at the second half of a `日`.
    ///
    /// Cell position is not strictly increasing — combining marks and
    /// variation selectors occupy no columns — so several bytes can share a
    /// cell. The **first** of them is returned, which is the one a renderer
    /// should start drawing from.
    ///
    /// Cells past the end of the line clamp to its end.
    pub fn cell_to_byte(&self, cell: CellCol) -> ByteOff {
        match &self.index {
            Index::Trivial { len } => ByteOff(cell.get().min(*len)),
            Index::Mapped(stops) => {
                let target = cell.get();
                let last = self.last();
                // Strictly past the last column: clamp to the end of the line.
                // This has to precede the rewind below, because a line ending
                // in a zero-width cluster shares its final cell with the bytes
                // before it, and would otherwise rewind onto them — reporting
                // the *start* of the line for a column past its end.
                if target > last.cell {
                    return ByteOff(last.byte);
                }
                // The greatest cell at or before the target.
                let at_or_before = partition(stops, |s| s.cell <= target);
                let found = stops[at_or_before].cell;
                // Rewind over any zero-width run sharing that cell.
                let first = stops.partition_point(|s| s.cell < found);
                ByteOff(stops[first].byte)
            }
        }
    }

    /// Character index of a byte offset.
    pub fn byte_to_char(&self, off: ByteOff) -> CharIdx {
        let end = (off.get() as usize).min(self.text.len());
        let end = floor_boundary(self.text, end);
        CharIdx(self.text[..end].chars().count() as u32)
    }

    /// Every grapheme cluster in the line, in order.
    pub fn graphemes(&self) -> impl Iterator<Item = Grapheme<'a>> + '_ {
        graphemes_from(self.text, self.tab_width, Position::ORIGIN)
    }

    /// Grapheme clusters overlapping a half-open range of cells.
    ///
    /// Used for horizontal scrolling. Clusters straddling either edge are
    /// included, so the caller can decide whether to clip or pad them.
    pub fn graphemes_in_cells(
        &self,
        cells: std::ops::Range<CellCol>,
    ) -> impl Iterator<Item = Grapheme<'a>> + '_ {
        let (start, end) = (cells.start.get(), cells.end.get());
        graphemes_from(self.text, self.tab_width, self.stop_before_cell(start))
            .skip_while(move |g| g.cells().end <= start)
            .take_while(move |g| g.cell.get() < end)
    }

    /// A grapheme boundary no later than the first cluster reaching `cell`.
    ///
    /// Scrolling right on a long line should not have to walk it from column
    /// zero. The result may sit one cluster early, since a wide character
    /// starting before `cell` still reaches it; the caller trims.
    fn stop_before_cell(&self, cell: u32) -> Position {
        match &self.index {
            Index::Trivial { len } => {
                let at = cell.min(*len);
                Position {
                    byte: at,
                    utf16: at,
                    cell: at,
                }
            }
            Index::Mapped(stops) => {
                let first_at_or_after = stops.partition_point(|s| s.cell < cell);
                stops[first_at_or_after.saturating_sub(1)]
            }
        }
    }

    fn last(&self) -> Position {
        match &self.index {
            Index::Trivial { len } => Position {
                byte: *len,
                utf16: *len,
                cell: *len,
            },
            Index::Mapped(stops) => *stops.last().expect("stops always has a terminal entry"),
        }
    }
}

/// Printable ASCII with no tabs, where every coordinate system agrees.
fn is_trivial(text: &str) -> bool {
    text.bytes().all(|b| (0x20..0x7f).contains(&b))
}

fn build_positions(text: &str, tab_width: u8) -> Vec<Position> {
    // One stop per cluster plus a terminal entry. Clusters never outnumber
    // characters, and counting them is far cheaper than the segmentation
    // below — whereas sizing by byte length over-allocates threefold on CJK.
    let mut stops = Vec::with_capacity(text.chars().count() + 1);
    let (mut byte, mut utf16, mut cell) = (0u32, 0u32, 0u32);
    for grapheme in text.graphemes(true) {
        stops.push(Position { byte, utf16, cell });
        byte = byte.saturating_add(len32(grapheme));
        utf16 = utf16.saturating_add(utf16_len(grapheme));
        cell = cell.saturating_add(if grapheme == "\t" {
            tab_advance(CellCol(cell), tab_width)
        } else {
            grapheme_width(grapheme)
        });
    }
    stops.push(Position { byte, utf16, cell });
    stops
}

/// Index of the last stop satisfying `pred`, which is monotone over the table.
fn partition(stops: &[Position], pred: impl Fn(&Position) -> bool) -> usize {
    stops.partition_point(|s| pred(s)).saturating_sub(1)
}

/// Largest character boundary at or below `at`.
fn floor_boundary(text: &str, mut at: usize) -> usize {
    while at > 0 && !text.is_char_boundary(at) {
        at -= 1;
    }
    at
}
