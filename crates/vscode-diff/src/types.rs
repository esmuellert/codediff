//! Owned Rust representations of a computed diff.
//!
//! These carry no borrows and no C pointers, so a `Diff` is an ordinary value:
//! it can be stored, sent between threads and outlive the call that produced it.
//!
//! Index conventions are inherited from the engine, which mirrors VSCode:
//!
//! - lines are **1-based**, ranges are **end-exclusive**
//! - columns are **1-based** and counted in **UTF-16 code units**, not bytes

/// A range of lines: 1-based, `start` inclusive, `end` exclusive.
///
/// An empty range (`start == end`) is meaningful: it marks the position where
/// text was inserted or removed on the other side.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct LineRange {
    pub start: u32,
    pub end: u32,
}

impl LineRange {
    /// True when this side contributes no lines, i.e. the change is purely an
    /// insertion or deletion on the other side.
    pub fn is_empty(&self) -> bool {
        self.start >= self.end
    }

    /// Number of lines covered.
    pub fn len(&self) -> u32 {
        self.end.saturating_sub(self.start)
    }
}

/// A position range within a line: 1-based, `end_col` exclusive, columns in
/// UTF-16 code units.
///
/// Converting these to byte offsets is the `metrics` crate's job; doing it
/// naively will misplace highlights on any non-ASCII line.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CharRange {
    pub start_line: u32,
    pub start_col: u32,
    pub end_line: u32,
    pub end_col: u32,
}

/// A character-level correspondence between the two sides of a change.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RangeMapping {
    pub original: CharRange,
    pub modified: CharRange,
}

/// A line-level change, refined by character-level detail where the engine
/// found any.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Change {
    pub original: LineRange,
    pub modified: LineRange,
    /// Empty when the engine reported no character-level detail, which is
    /// normal for whole-line insertions and deletions.
    pub inner: Vec<RangeMapping>,
}

impl Change {
    /// True when lines exist only on the modified side.
    pub fn is_insertion(&self) -> bool {
        self.original.is_empty() && !self.modified.is_empty()
    }

    /// True when lines exist only on the original side.
    pub fn is_deletion(&self) -> bool {
        self.modified.is_empty() && !self.original.is_empty()
    }
}

/// A block of lines the engine judged to have moved rather than been deleted
/// and re-added. Only produced when [`crate::Options::compute_moves`] is set.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Move {
    pub original: LineRange,
    pub modified: LineRange,
}

/// The result of comparing two texts.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Diff {
    pub changes: Vec<Change>,
    pub moves: Vec<Move>,
    /// True when the engine stopped early because it exceeded
    /// [`crate::Options::max_computation_time_ms`]. The diff is still valid,
    /// but coarser than it would otherwise have been.
    pub hit_timeout: bool,
}

impl Diff {
    /// True when the two texts are identical.
    pub fn is_empty(&self) -> bool {
        self.changes.is_empty()
    }
}
