//! The layer's one public entry point.

use diff_types::{DetailedLineRangeMapping, LineRange, LinesDiff, MovedText};
pub use file_types::DiffVersion;

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

/// A diff paired up with the two files it came from.
///
/// **Owns** all three, and is therefore a plain value: it can be returned from
/// a function, stored in a struct, and moved into a collection. That is the
/// whole reason it owns them. A borrowing version cannot be returned by the
/// stage that builds it — the texts it points at die when that function ends —
/// so the pipeline had to lend it through a closure, and every type that held
/// one grew a lifetime parameter: `Diff<'a>`, `Session<'a>`, `View<'a>`, and so
/// on down. See D27.
///
/// The cost is one copy of each file, once, when the alignment is built. There
/// is still exactly one copy in existence — the lines are copied *in* and the
/// caller's can be dropped — so the original reason for borrowing, that no
/// second copy can fall out of step with the first, still holds.
///
/// Everything below is computed when asked, except [`hunks`](Self::hunks),
/// which is O(changes) and built once here.
#[derive(Debug, Clone)]
pub struct Alignment {
    diff: LinesDiff,
    original: Vec<String>,
    modified: Vec<String>,
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
        context: u32,
    ) -> Self {
        Self::try_with_options(diff, original, modified, tab_width, context)
            .expect("the engine produces well-formed diffs")
    }

    pub fn try_with_options(
        diff: LinesDiff,
        original: &[&str],
        modified: &[&str],
        tab_width: u8,
        context: u32,
    ) -> Result<Self, Malformed> {
        let original = normalise(original);
        let modified = normalise(modified);
        if !is_well_formed(&diff, original.len() as u32, modified.len() as u32) {
            return Err(Malformed);
        }
        let hunks = hunks(&diff, &original, &modified, context);
        Ok(Self {
            diff,
            original,
            modified,
            tab_width,
            hunks,
        })
    }

    /// The changed blocks, in order.
    ///
    /// These and the two below are the engine's result, borrowed rather than
    /// restated: `Alignment` holds a `&LinesDiff` and reads through it. VSCode
    /// unpacks the same four values into its `DiffState` and drops the result,
    /// so a caller there writes `state.movedTexts` and there is no diff object
    /// left to reach into. Borrowing is free where copying is not, so the
    /// surface matches without the copy.
    pub fn changes(&self) -> &[DetailedLineRangeMapping] {
        &self.diff.changes
    }

    /// Blocks the engine judged to have moved rather than been rewritten.
    ///
    /// Empty unless the diff was computed with [`Options::with_moves`].
    ///
    /// [`Options::with_moves`]: vscode_diff::Options::with_moves
    pub fn moves(&self) -> &[MovedText] {
        &self.diff.moves
    }

    /// The engine gave up before finishing, so the pairing is coarser than the
    /// files warrant.
    ///
    /// What is shown is still valid, but incomplete — a reviewer who mistakes
    /// it for a finished diff approves code they have not seen, so it must
    /// reach the screen. VSCode calls this `quitEarly`.
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

    /// The text of one line, numbered from 1.
    pub fn line(&self, version: DiffVersion, number: u32) -> Option<&str> {
        self.lines(version)
            .get(number.checked_sub(1)? as usize)
            .map(String::as_str)
    }

    /// Every row, in order.
    pub fn rows(&self) -> Rows<'_> {
        rows(
            &self.diff,
            self.original.len() as u32,
            self.modified.len() as u32,
        )
    }

    pub fn row_count(&self) -> u32 {
        row_count(
            &self.diff,
            self.original.len() as u32,
            self.modified.len() as u32,
        )
    }

    /// The rows a viewport covers, without walking the ones above it.
    ///
    /// Still a walk — the rows before `first` are stepped over rather than
    /// built. For the change counts real files produce this costs nothing, and
    /// it keeps the alternative, a stored row index, out of the crate.
    pub fn rows_from(&self, first: u32) -> impl Iterator<Item = Row> + '_ {
        self.rows().skip(first as usize)
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

    /// Character-level changes on one line, as byte ranges into it.
    ///
    /// An inner change can span several lines, so this asks for one line at a
    /// time rather than returning a shape the caller has to unpick.
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

    /// Runs of lines identical on both sides.
    pub fn unchanged(&self) -> Vec<UnchangedRegion> {
        regions(
            &self.diff,
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

/// An empty file, as the engine models it.
///
/// `vscode_diff::compute` turns `&[]` into `&[""]` before handing it to the
/// engine, so a diff of an empty file talks about line 1. An `Alignment` given
/// the un-normalised `&[]` would hold a file with no line 1 and disagree with
/// its own diff, so it normalises identically. Found by `proptest`, which
/// shrank to `original = []`.
/// Copies the caller's lines in, standing an absent file up as the engine's
/// representation of an empty one: a single empty line.
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
