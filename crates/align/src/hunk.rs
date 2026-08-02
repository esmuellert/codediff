//! Hunks: nearby changes grouped, with an identity that survives a refresh.
//!
//! A reviewer marks a hunk as read. The file is then saved, re-diffed, and
//! every `DetailedLineRangeMapping` in it is a fresh object at a possibly different line number.
//! [`HunkId`] is derived from the hunk's own text, so a hunk that did not change
//! keeps its identity and its mark, while one that did gets a new id and comes
//! back unread — which is the behaviour you want.

use std::collections::HashMap;

use diff_types::{DetailedLineRangeMapping, LineRange, LinesDiff};

/// Unchanged lines allowed inside one hunk before it splits in two.
pub const DEFAULT_CONTEXT: u32 = 3;

/// Identity of a hunk, derived from its content rather than its position.
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
/// Two changes separated by more than `context` unchanged lines are read as
/// separate edits and get separate hunks.
pub fn hunks<S: AsRef<str>>(
    diff: &LinesDiff,
    original: &[S],
    modified: &[S],
    context: u32,
) -> Vec<Hunk> {
    let mut out = Vec::new();
    // How many hunks with identical text have been seen already, so that two
    // of them can be told apart. See `identity`.
    let mut occurrences: HashMap<u64, u32> = HashMap::new();
    let mut start = 0usize;

    for i in 1..diff.changes.len() {
        if gap(&diff.changes[i - 1], &diff.changes[i]) > context {
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

/// Hashes what the hunk *says*, not where it sits.
///
/// Line numbers are deliberately excluded: inserting an unrelated function
/// above a hunk moves it without changing it, and a reviewer should not have to
/// read it again.
///
/// Content alone is not unique, though. The same edit made twice in one file
/// hashes identically both times, and one review mark would then cover both, so
/// the number of identical hunks seen so far is mixed in. Ids stay independent
/// of line numbers while becoming distinct within a file.
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

/// FNV-1a, spelled out rather than taken from `DefaultHasher`.
///
/// `DefaultHasher`'s algorithm is explicitly unspecified and may change between
/// Rust releases. A `HunkId` is printed into the golden snapshots and is the
/// obvious key for review state that might one day outlive a process, so it has
/// to mean the same thing next year as it does today.
fn fnv1a(bytes: &[u8], mut hash: u64) -> u64 {
    for byte in bytes {
        hash ^= u64::from(*byte);
        hash = hash.wrapping_mul(FNV_PRIME);
    }
    hash
}
