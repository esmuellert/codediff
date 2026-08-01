//! The layer's one public entry point.

use vscode_diff::{LineRange, LinesDiff, MovedText};

use crate::hunk::{DEFAULT_CONTEXT, Hunk, HunkId, hunks};
use crate::inner::{Span, span_on};
use crate::region::{UnchangedRegion, regions};
use crate::row::{Row, Rows, is_well_formed, row_count, rows};

/// A diff whose ranges do not describe a coherent pairing.
///
/// Only reachable through an engine bug: the ranges must be ordered,
/// non-overlapping, inside their files, and must leave both sides with the same
/// number of unchanged lines. Reported rather than ignored because the
/// alternative is a review tool quietly showing a line twice.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Malformed;

impl std::fmt::Display for Malformed {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("the diff's line ranges do not describe a coherent pairing")
    }
}

impl std::error::Error for Malformed {}

/// Which file a line number refers to.
///
/// Deliberately not `Left` and `Right`. Those are places on a screen, and
/// inline view puts both on the same side; a model that names them cannot
/// describe it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Side {
    Original,
    Modified,
}

/// A diff paired up with the two files it came from.
///
/// Borrows all three and copies none of them. Every answer below is computed
/// when asked, so there is no second copy of the document to fall out of step
/// with the first.
#[derive(Debug, Clone)]
pub struct Alignment<'a> {
    diff: &'a LinesDiff,
    original: &'a [&'a str],
    modified: &'a [&'a str],
    tab_width: u8,
    hunks: Vec<Hunk>,
}

impl<'a> Alignment<'a> {
    /// Pairs a diff with the files it came from.
    ///
    /// # Panics
    ///
    /// If the diff does not describe a coherent pairing. Use
    /// [`try_new`](Self::try_new) to be told instead.
    pub fn new(diff: &'a LinesDiff, original: &'a [&'a str], modified: &'a [&'a str]) -> Self {
        Self::try_new(diff, original, modified).expect("the engine produces well-formed diffs")
    }

    pub fn try_new(
        diff: &'a LinesDiff,
        original: &'a [&'a str],
        modified: &'a [&'a str],
    ) -> Result<Self, Malformed> {
        Self::try_with_options(
            diff,
            original,
            modified,
            line_index::DEFAULT_TAB_WIDTH,
            DEFAULT_CONTEXT,
        )
    }

    /// # Panics
    ///
    /// As [`new`](Self::new).
    pub fn with_options(
        diff: &'a LinesDiff,
        original: &'a [&'a str],
        modified: &'a [&'a str],
        tab_width: u8,
        context: u32,
    ) -> Self {
        Self::try_with_options(diff, original, modified, tab_width, context)
            .expect("the engine produces well-formed diffs")
    }

    pub fn try_with_options(
        diff: &'a LinesDiff,
        original: &'a [&'a str],
        modified: &'a [&'a str],
        tab_width: u8,
        context: u32,
    ) -> Result<Self, Malformed> {
        let original = normalise(original);
        let modified = normalise(modified);
        if !is_well_formed(diff, original.len() as u32, modified.len() as u32) {
            return Err(Malformed);
        }
        Ok(Self {
            diff,
            original,
            modified,
            tab_width,
            hunks: hunks(diff, original, modified, context),
        })
    }

    pub fn diff(&self) -> &'a LinesDiff {
        self.diff
    }

    pub fn lines(&self, side: Side) -> &'a [&'a str] {
        match side {
            Side::Original => self.original,
            Side::Modified => self.modified,
        }
    }

    /// The text of one line, numbered from 1.
    pub fn line(&self, side: Side, number: u32) -> Option<&'a str> {
        self.lines(side)
            .get(number.checked_sub(1)? as usize)
            .copied()
    }

    /// Every row, in order.
    pub fn rows(&self) -> Rows<'a> {
        rows(
            self.diff,
            self.original.len() as u32,
            self.modified.len() as u32,
        )
    }

    pub fn row_count(&self) -> u32 {
        row_count(
            self.diff,
            self.original.len() as u32,
            self.modified.len() as u32,
        )
    }

    /// The rows a viewport covers, without walking the ones above it.
    ///
    /// Still a walk — the rows before `first` are stepped over rather than
    /// built. For the change counts real files produce this costs nothing, and
    /// it keeps the alternative, a stored row index, out of the crate.
    pub fn rows_from(&self, first: u32) -> impl Iterator<Item = Row> + 'a {
        self.rows().skip(first as usize)
    }

    pub fn hunks(&self) -> &[Hunk] {
        &self.hunks
    }

    pub fn hunk(&self, id: HunkId) -> Option<&Hunk> {
        self.hunks.iter().find(|h| h.id == id)
    }

    /// The hunk a line belongs to, if any.
    pub fn hunk_at(&self, side: Side, line: u32) -> Option<&Hunk> {
        self.hunks.iter().find(|h| contains(range(h, side), line))
    }

    /// Character-level changes on one line, as byte ranges into it.
    ///
    /// An inner change can span several lines, so this asks for one line at a
    /// time rather than returning a shape the caller has to unpick.
    pub fn spans(&self, side: Side, line: u32) -> Vec<Span> {
        let lines = self.lines(side);
        self.diff
            .changes
            .iter()
            .filter(|change| contains(line_range(change, side), line))
            .flat_map(|change| &change.inner_changes)
            .map(|mapping| match side {
                Side::Original => &mapping.original,
                Side::Modified => &mapping.modified,
            })
            // Expanding a mapping walks every line it touches, so skip the ones
            // that cannot contribute before doing that work.
            .filter(|range| line >= range.start_line && line <= range.end_line)
            .flat_map(|range| span_on(range, line, lines, self.tab_width))
            .collect()
    }

    /// Runs of lines identical on both sides.
    pub fn unchanged(&self) -> Vec<UnchangedRegion> {
        regions(
            self.diff,
            self.original.len() as u32,
            self.modified.len() as u32,
        )
    }

    /// The move a line takes part in, if any.
    ///
    /// A lookup rather than a field on the row: the engine's move ranges need
    /// not agree with its change ranges — in the `comprehensive_move` fixture a
    /// move covers original 32..89 while a change covers 37..139 — so a move
    /// cannot be attached to a change without lying about one of them.
    pub fn moved(&self, side: Side, line: u32) -> Option<&'a MovedText> {
        self.diff
            .moves
            .iter()
            .find(|m| contains(move_range(m, side), line))
    }
}

fn contains(range: LineRange, line: u32) -> bool {
    line >= range.start_line && line < range.end_line
}

/// An empty file, as the engine models it.
///
/// `vscode_diff::compute` turns `&[]` into `&[""]` before handing it to the
/// engine, so a diff of an empty file talks about line 1. An `Alignment` given
/// the un-normalised `&[]` would hold a file with no line 1 and disagree with
/// its own diff, so it normalises identically. Found by `proptest`, which
/// shrank to `original = []`.
fn normalise<'a>(lines: &'a [&'a str]) -> &'a [&'a str] {
    const EMPTY_FILE: &[&str] = &[""];
    if lines.is_empty() { EMPTY_FILE } else { lines }
}

fn range(hunk: &Hunk, side: Side) -> LineRange {
    match side {
        Side::Original => hunk.original,
        Side::Modified => hunk.modified,
    }
}

fn line_range(change: &vscode_diff::DetailedLineRangeMapping, side: Side) -> LineRange {
    match side {
        Side::Original => change.original,
        Side::Modified => change.modified,
    }
}

fn move_range(moved: &MovedText, side: Side) -> LineRange {
    match side {
        Side::Original => moved.original,
        Side::Modified => moved.modified,
    }
}
