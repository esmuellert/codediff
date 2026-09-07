//! Converts C results to owned Rust values and frees C memory.
#![allow(unsafe_code)]

use vscode_diff_sys as sys;

use diff_types::{
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

    // SAFETY: the caller provides a live result; all fields are copied before freeing it.
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

    // SAFETY: `raw` is still live and all references to it are gone.
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

/// Clamps engine line values to non-negative `u32` values.
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
