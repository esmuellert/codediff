//! Groups nearby changes into hunks with stable content-based IDs.

use std::collections::HashMap;

use diff_types::{DetailedLineRangeMapping, LineRange, LinesDiff};

/// Unchanged lines allowed inside one hunk before it splits in two.
pub const DEFAULT_CONTEXT: u32 = 3;

/// Stable hunk identity derived from its content.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct HunkId(pub u64);

/// A group of changes close enough to read as one edit.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Hunk {
    pub id: HunkId,
    /// Which changes this covers, as indices into [`LinesDiff::changes`].
    pub changes: std::ops::Range<usize>,
    pub original: LineRange,
    pub modified: LineRange,
}

/// Groups the changes into hunks.
///
/// Two changes separated by more than `keymap_type` unchanged lines are read as
/// separate edits and get separate hunks.
pub fn hunks<S: AsRef<str>>(
    diff: &LinesDiff,
    original: &[S],
    modified: &[S],
    keymap_type: u32,
) -> Vec<Hunk> {
    let mut out = Vec::new();
    // How many hunks with identical text have been seen already, so that two
    // of them can be told apart. See `identity`.
    let mut occurrences: HashMap<u64, u32> = HashMap::new();
    let mut start = 0usize;

    for i in 1..diff.changes.len() {
        if gap(&diff.changes[i - 1], &diff.changes[i]) > keymap_type {
            out.push(build(diff, start..i, original, modified, &mut occurrences));
            start = i;
        }
    }
    if !diff.changes.is_empty() {
        out.push(build(
            diff,
            start..diff.changes.len(),
            original,
            modified,
            &mut occurrences,
        ));
    }
    out
}

/// Unchanged lines between two consecutive changes.
fn gap(previous: &DetailedLineRangeMapping, next: &DetailedLineRangeMapping) -> u32 {
    next.original
        .start_line
        .saturating_sub(previous.original.end_line)
}

fn build<S: AsRef<str>>(
    diff: &LinesDiff,
    changes: std::ops::Range<usize>,
    original: &[S],
    modified: &[S],
    occurrences: &mut HashMap<u64, u32>,
) -> Hunk {
    let group = &diff.changes[changes.clone()];
    let first = group.first().expect("a hunk holds at least one change");
    let last = group.last().expect("a hunk holds at least one change");

    let original_range = LineRange {
        start_line: first.original.start_line,
        end_line: last.original.end_line,
    };
    let modified_range = LineRange {
        start_line: first.modified.start_line,
        end_line: last.modified.end_line,
    };

    Hunk {
        id: identity(
            original_range,
            modified_range,
            original,
            modified,
            occurrences,
        ),
        changes,
        original: original_range,
        modified: modified_range,
    }
}

/// Hashes the hunk's content, not its position. Line numbers excluded so
/// moving code doesn't invalidate review state. A sequence counter makes
/// duplicate edits in one file distinct.
fn identity<S: AsRef<str>>(
    original: LineRange,
    modified: LineRange,
    original_lines: &[S],
    modified_lines: &[S],
    occurrences: &mut HashMap<u64, u32>,
) -> HunkId {
    let mut hash = FNV_OFFSET;
    for text in slice(original_lines, original) {
        hash = fnv1a(text.as_ref().as_bytes(), hash);
        // Without a separator, ["ab", "c"] and ["a", "bc"] hash alike.
        hash = fnv1a(&[0xff], hash);
    }
    hash = fnv1a(&[0xfe], hash);
    for text in slice(modified_lines, modified) {
        hash = fnv1a(text.as_ref().as_bytes(), hash);
        hash = fnv1a(&[0xff], hash);
    }

    let seen = occurrences.entry(hash).or_insert(0);
    let id = fnv1a(&seen.to_le_bytes(), hash);
    *seen += 1;
    HunkId(id)
}

fn slice<S>(lines: &[S], range: LineRange) -> &[S] {
    let start = range.start_line.saturating_sub(1) as usize;
    let end = range.end_line.saturating_sub(1) as usize;
    let start = start.min(lines.len());
    let end = end.clamp(start, lines.len());
    &lines[start..end]
}

const FNV_OFFSET: u64 = 0xcbf2_9ce4_8422_2325;
const FNV_PRIME: u64 = 0x0000_0100_0000_01b3;

/// Deterministic FNV-1a hash used for hunk IDs.
fn fnv1a(bytes: &[u8], mut hash: u64) -> u64 {
    for byte in bytes {
        hash ^= u64::from(*byte);
        hash = hash.wrapping_mul(FNV_PRIME);
    }
    hash
}
