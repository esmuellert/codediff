//! The single place that dereferences pointers returned by the C engine.
//!
//! The rule is *convert eagerly, free immediately*: the result is walked once
//! into owned Rust values and released before returning, so no raw pointer ever
//! escapes into application types. That keeps the unsafe surface small enough
//! to read in full, instead of spreading a lifetime obligation across the
//! program.
#![allow(unsafe_code)]

use vscode_diff_sys as sys;

use crate::types::{
    CharRange, DetailedLineRangeMapping, LineRange, LinesDiff, MovedText, RangeMapping,
};

/// Takes ownership of a `LinesDiff`, copies it into owned Rust values, and
/// frees the C allocation.
///
/// # Safety
///
/// `raw` must be a non-null pointer returned by `sys::compute_diff` that has
/// not already been freed. It is freed here, so the caller must not use it
/// afterwards.
pub(crate) unsafe fn take(raw: *mut sys::LinesDiff) -> LinesDiff {
    debug_assert!(!raw.is_null(), "take() requires a non-null LinesDiff");

    // SAFETY: the caller guarantees `raw` is a live, non-null result from
    // `compute_diff`. Nothing below can panic before the free: every operation
    // is a copy of plain integers, and `Vec` growth aborts rather than unwinds
    // on allocation failure.
    let (changes, moves, hit_timeout) = unsafe {
        let diff = &*raw;

        let mut changes = Vec::with_capacity(diff.changes.count.max(0) as usize);
        for i in 0..diff.changes.count.max(0) as isize {
            let mapping = &*diff.changes.mappings.offset(i);
            changes.push(DetailedLineRangeMapping {
                original: line_range(mapping.original),
                modified: line_range(mapping.modified),
                inner_changes: inner_changes(mapping.inner_changes, mapping.inner_change_count),
            });
        }

        let mut moves = Vec::with_capacity(diff.moves.count.max(0) as usize);
        for i in 0..diff.moves.count.max(0) as isize {
            let moved = &*diff.moves.moves.offset(i);
            moves.push(MovedText {
                original: line_range(moved.original),
                modified: line_range(moved.modified),
            });
        }

        (changes, moves, diff.hit_timeout)
    };

    // SAFETY: `raw` came from `compute_diff` and has not been freed. Everything
    // above is copied, so releasing it now leaves no dangling reference.
    unsafe { sys::free_lines_diff(raw) };

    LinesDiff {
        changes,
        moves,
        hit_timeout,
    }
}

/// # Safety
///
/// `ptr` must either be null with `count <= 0`, or point to at least `count`
/// valid `RangeMapping` values.
unsafe fn inner_changes(ptr: *mut sys::RangeMapping, count: i32) -> Vec<RangeMapping> {
    if ptr.is_null() || count <= 0 {
        return Vec::new();
    }
    // SAFETY: guaranteed by the caller; `count` is the engine's own length.
    unsafe {
        (0..count as isize)
            .map(|i| {
                let mapping = &*ptr.offset(i);
                RangeMapping {
                    original: char_range(mapping.original),
                    modified: char_range(mapping.modified),
                }
            })
            .collect()
    }
}

/// The engine emits non-negative line numbers; clamping rather than wrapping
/// means a hypothetical negative would surface as an obviously wrong `0`
/// instead of a plausible four-billion.
fn line_range(range: sys::LineRange) -> LineRange {
    LineRange {
        start_line: non_negative(range.start_line),
        end_line: non_negative(range.end_line),
    }
}

fn char_range(range: sys::CharRange) -> CharRange {
    CharRange {
        start_line: non_negative(range.start_line),
        start_col: non_negative(range.start_col),
        end_line: non_negative(range.end_line),
        end_col: non_negative(range.end_col),
    }
}

fn non_negative(value: i32) -> u32 {
    u32::try_from(value).unwrap_or(0)
}
