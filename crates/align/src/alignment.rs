//! `Alignment`: borrows a diff and two files, answers positional queries.

use std::sync::Arc;

use diff_types::{DetailedLineRangeMapping, LineRange, LinesDiff, MovedText};
use file_types::DiffType;
pub use file_types::DiffVersion;

use crate::decoration::{LineDecorations, decorations};
use crate::hunk::{DEFAULT_CONTEXT, Hunk, HunkId, hunks};
use crate::inner::{Span, span_on};
use crate::layout::{self, ViewLines};
use crate::normalize::normalize_document_diff;
use crate::view_line::{ViewLine, blocks, is_well_formed};

/// A diff whose ranges cannot form a valid pairing.
///
/// Ranges must be ordered, non-overlapping, in bounds, and preserve unchanged-line counts.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Malformed;

impl std::fmt::Display for Malformed {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("the diff's line ranges do not describe a coherent pairing")
    }
}

impl std::error::Error for Malformed {}

/// A diff paired with the two files it describes. Owns all three.
///
/// Everything is computed on demand except [`hunks`](Self::hunks), which is
/// built once at construction.
#[derive(Debug, Clone)]
pub struct Alignment {
    diff: LinesDiff,
    /// Shared file text used by the colouring thread.
    original: Arc<Vec<String>>,
    modified: Arc<Vec<String>>,
    tab_width: u8,
    hunks: Vec<Hunk>,
}

impl Alignment {
    /// Pairs a diff with the files it came from.
    ///
    /// # Panics
    ///
    /// If the diff does not describe a coherent pairing. Use
    /// [`try_new`](Self::try_new) to be told instead.
    pub fn new(diff: LinesDiff, original: &[&str], modified: &[&str]) -> Self {
        Self::try_new(diff, original, modified).expect("the engine produces well-formed diffs")
    }

    pub fn try_new(
        diff: LinesDiff,
        original: &[&str],
        modified: &[&str],
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
        diff: LinesDiff,
        original: &[&str],
        modified: &[&str],
        tab_width: u8,
        keymap_type: u32,
    ) -> Self {
        Self::try_with_options(diff, original, modified, tab_width, keymap_type)
            .expect("the engine produces well-formed diffs")
    }

    pub fn try_with_options(
        diff: LinesDiff,
        original: &[&str],
        modified: &[&str],
        tab_width: u8,
        keymap_type: u32,
    ) -> Result<Self, Malformed> {
        let original = normalise(original);
        let modified = normalise(modified);
        let mut diff = diff;
        normalize_document_diff(&mut diff, &original, &modified);
        if !is_well_formed(&diff, original.len() as u32, modified.len() as u32) {
            return Err(Malformed);
        }
        let hunks = hunks(&diff, &original, &modified, keymap_type);
        Ok(Self {
            diff,
            original: Arc::new(original),
            modified: Arc::new(modified),
            tab_width,
            hunks,
        })
    }

    /// Changed line mappings in engine order.
    pub fn changes(&self) -> &[DetailedLineRangeMapping] {
        &self.diff.changes
    }

    /// Blocks the engine marked as moved.
    ///
    /// Empty when move detection was not requested.
    pub fn moves(&self) -> &[MovedText] {
        &self.diff.moves
    }

    /// Whether the engine stopped before producing a complete diff.
    ///
    /// The returned pairing may be coarse.
    pub fn hit_timeout(&self) -> bool {
        self.diff.hit_timeout
    }

    /// True when the two sides are identical.
    pub fn is_empty(&self) -> bool {
        self.diff.changes.is_empty()
    }

    pub fn lines(&self, version: DiffVersion) -> &[String] {
        match version {
            DiffVersion::Original => &self.original,
            DiffVersion::Modified => &self.modified,
        }
    }

    /// Returns shared text for one version.
    pub fn text(&self, version: DiffVersion) -> Arc<Vec<String>> {
        match version {
            DiffVersion::Original => Arc::clone(&self.original),
            DiffVersion::Modified => Arc::clone(&self.modified),
        }
    }

    /// The text of one line, numbered from 1.
    pub fn line(&self, version: DiffVersion, number: u32) -> Option<&str> {
        self.lines(version)
            .get(number.checked_sub(1)? as usize)
            .map(String::as_str)
    }

    /// Every view line, in order, laid out the way asked for.
    pub fn view_lines(&self, layout: DiffType) -> ViewLines<'_> {
        layout::view_lines(
            layout,
            &self.diff,
            &self.original,
            self.originals(),
            self.modifieds(),
        )
    }

    pub fn view_line_count(&self, layout: DiffType) -> u32 {
        layout::view_line_count(
            layout,
            &self.diff,
            &self.original,
            self.originals(),
            self.modifieds(),
        )
    }

    /// Iterates view lines starting at `first`.
    pub fn view_lines_from(
        &self,
        layout: DiffType,
        first: u32,
    ) -> impl Iterator<Item = ViewLine> + '_ {
        self.view_lines(layout).skip(first as usize)
    }

    /// Runs of adjacent changed view lines, in the given view layout.
    pub fn blocks(&self, layout: DiffType) -> Vec<std::ops::Range<u32>> {
        blocks(self.view_lines(layout))
    }

    /// Returns the file line shown by a view line.
    ///
    /// File line numbers are shared by both layouts.
    pub fn line_at(&self, layout: DiffType, view_line: u32) -> Option<(DiffVersion, u32)> {
        layout::line_at(
            layout,
            &self.diff,
            &self.original,
            self.originals(),
            self.modifieds(),
            view_line,
        )
    }

    /// Which line shows a given line, if any.
    pub fn view_line_at(&self, layout: DiffType, version: DiffVersion, line: u32) -> Option<u32> {
        layout::view_line_at(
            layout,
            &self.diff,
            &self.original,
            self.originals(),
            self.modifieds(),
            version,
            line,
        )
    }

    fn originals(&self) -> u32 {
        self.original.len() as u32
    }

    fn modifieds(&self) -> u32 {
        self.modified.len() as u32
    }

    pub fn hunks(&self) -> &[Hunk] {
        &self.hunks
    }

    pub fn hunk(&self, id: HunkId) -> Option<&Hunk> {
        self.hunks.iter().find(|h| h.id == id)
    }

    /// The hunk a line belongs to, if any.
    pub fn hunk_at(&self, version: DiffVersion, line: u32) -> Option<&Hunk> {
        self.hunks
            .iter()
            .find(|h| contains(range(h, version), line))
    }

    /// Returns byte spans for inner changes on one line.
    pub fn spans(&self, version: DiffVersion, line: u32) -> Vec<Span> {
        let lines = self.lines(version);
        self.diff
            .changes
            .iter()
            .filter(|change| contains(line_range(change, version), line))
            .flat_map(|change| &change.inner_changes)
            .map(|mapping| match version {
                DiffVersion::Original => &mapping.original,
                DiffVersion::Modified => &mapping.modified,
            })
            // Expanding a mapping walks every line it touches, so skip the ones
            // that cannot contribute before doing that work.
            .filter(|range| line >= range.start_line && line <= range.end_line)
            .flat_map(|range| span_on(range, line, lines, self.tab_width))
            .collect()
    }

    /// Returns line backgrounds, character ranges, and empty markers.
    pub fn decorations(&self, version: DiffVersion, line: u32) -> LineDecorations {
        decorations(
            &self.diff,
            self.lines(version),
            self.tab_width,
            version,
            line,
        )
    }

    /// Returns the move containing a line, if any.
    pub fn moved(&self, version: DiffVersion, line: u32) -> Option<&MovedText> {
        self.diff
            .moves
            .iter()
            .find(|m| contains(move_range(m, version), line))
    }
}

fn contains(range: LineRange, line: u32) -> bool {
    line >= range.start_line && line < range.end_line
}

/// Copies the lines and normalizes an empty side to one empty line, matching
/// the engine's document model.
fn normalise(lines: &[&str]) -> Vec<String> {
    if lines.is_empty() {
        return vec![String::new()];
    }
    lines.iter().map(|line| (*line).to_owned()).collect()
}

fn range(hunk: &Hunk, version: DiffVersion) -> LineRange {
    match version {
        DiffVersion::Original => hunk.original,
        DiffVersion::Modified => hunk.modified,
    }
}

fn line_range(change: &diff_types::DetailedLineRangeMapping, version: DiffVersion) -> LineRange {
    match version {
        DiffVersion::Original => change.original,
        DiffVersion::Modified => change.modified,
    }
}

fn move_range(moved: &MovedText, version: DiffVersion) -> LineRange {
    match version {
        DiffVersion::Original => moved.original,
        DiffVersion::Modified => moved.modified,
    }
}
