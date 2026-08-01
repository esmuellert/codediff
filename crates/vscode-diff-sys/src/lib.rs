//! Raw FFI bindings to the vendored libvscode-diff C engine.
//!
//! This crate is a faithful, unsafe, 1:1 transcription of the C ABI and nothing
//! more. It is the only crate in the workspace where `unsafe` is permitted;
//! ownership, `Result` and `Drop` belong to the `vscode-diff` crate above it.
//!
//! Layouts mirror `vendor/libvscode-diff/include/types.h` exactly. `bool` in
//! that header is C99 `_Bool`, which matches Rust's `bool` on every platform
//! this project targets.
//!
//! Index conventions, carried unchanged from the C:
//!
//! - `LineRange` is 1-indexed, start inclusive, end exclusive.
//! - `CharRange` is 1-indexed; `end_col` is exclusive. Columns are **UTF-16
//!   code units**, because the algorithm mirrors VSCode, whose strings are
//!   UTF-16. Converting them to byte offsets is `metrics`' job.

#![allow(non_camel_case_types)]

use std::ffi::c_char;
use std::os::raw::c_int;

/// A range of lines. 1-indexed; `start_line` inclusive, `end_line` exclusive.
#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct LineRange {
    pub start_line: c_int,
    pub end_line: c_int,
}

/// A range of characters. 1-indexed; `end_col` exclusive, columns in UTF-16
/// code units.
#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CharRange {
    pub start_line: c_int,
    pub start_col: c_int,
    pub end_line: c_int,
    pub end_col: c_int,
}

/// A character-level correspondence between the two sides.
#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RangeMapping {
    pub original: CharRange,
    pub modified: CharRange,
}

#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct RangeMappingArray {
    pub mappings: *mut RangeMapping,
    pub count: c_int,
    pub capacity: c_int,
}

/// A line-level change, optionally refined by character-level inner changes.
///
/// `inner_changes` is null when there are none; `inner_change_count` is then 0.
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct DetailedLineRangeMapping {
    pub original: LineRange,
    pub modified: LineRange,
    pub inner_changes: *mut RangeMapping,
    pub inner_change_count: c_int,
}

#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct DetailedLineRangeMappingArray {
    pub mappings: *mut DetailedLineRangeMapping,
    pub count: c_int,
    pub capacity: c_int,
}

/// A block of lines detected as moved rather than deleted and re-added.
#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MovedText {
    pub original: LineRange,
    pub modified: LineRange,
}

#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct MovedTextArray {
    pub moves: *mut MovedText,
    pub count: c_int,
    pub capacity: c_int,
}

/// Tuning for a single `compute_diff` call.
///
/// `max_computation_time_ms` of 0 means no timeout.
#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DiffOptions {
    pub ignore_trim_whitespace: bool,
    pub max_computation_time_ms: c_int,
    pub compute_moves: bool,
    pub extend_to_subwords: bool,
}

/// The result of a diff. Heap-allocated by the C; free with [`free_lines_diff`].
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct LinesDiff {
    pub changes: DetailedLineRangeMappingArray,
    pub moves: MovedTextArray,
    pub hit_timeout: bool,
}

unsafe extern "C" {
    /// Computes the diff between two arrays of NUL-terminated lines.
    ///
    /// Returns a heap-allocated `LinesDiff`, or null on allocation failure.
    /// The caller owns the result and must release it with [`free_lines_diff`].
    ///
    /// # Representing an empty file
    ///
    /// An empty file is **one empty line**, `[""]`, not zero lines. The engine
    /// mirrors VSCode's document model, in which every document has at least
    /// one line, and it has a dedicated full-file path keyed on
    /// `count == 1 && strlen(lines[0]) == 0`.
    ///
    /// Passing a count of 0 does not error: it silently returns no changes, so
    /// a file whose entire content was added would appear unchanged. Callers
    /// must normalise an empty side to `[""]`.
    ///
    /// # Safety
    ///
    /// `original_lines` and `modified_lines` must point to arrays of at least
    /// `original_count` and `modified_count` valid, NUL-terminated pointers,
    /// which must remain valid for the duration of the call. `options` must be
    /// a valid pointer.
    ///
    /// Rust string slices are not NUL-terminated; callers must marshal through
    /// `CString` or equivalent.
    pub fn compute_diff(
        original_lines: *const *const c_char,
        original_count: c_int,
        modified_lines: *const *const c_char,
        modified_count: c_int,
        options: *const DiffOptions,
    ) -> *mut LinesDiff;

    /// Releases a `LinesDiff` and everything it owns.
    ///
    /// # Safety
    ///
    /// `diff` must have come from [`compute_diff`] and must not have been freed
    /// already. Passing null is permitted.
    pub fn free_lines_diff(diff: *mut LinesDiff);

    /// The engine's version string, statically allocated. Never null.
    pub fn get_version() -> *const c_char;
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::ffi::{CStr, CString};
    use std::mem::{align_of, size_of};

    #[test]
    fn layouts_match_the_c_header() {
        // Every field is a C int, so these are pure arithmetic and would fail
        // loudly if a field were added, removed or reordered.
        assert_eq!(size_of::<LineRange>(), 2 * size_of::<c_int>());
        assert_eq!(size_of::<CharRange>(), 4 * size_of::<c_int>());
        assert_eq!(size_of::<RangeMapping>(), 2 * size_of::<CharRange>());
        assert_eq!(size_of::<MovedText>(), 2 * size_of::<LineRange>());
        assert_eq!(align_of::<LineRange>(), align_of::<c_int>());
    }

    #[test]
    fn version_is_readable_through_the_ffi() {
        let version = unsafe {
            let ptr = get_version();
            assert!(!ptr.is_null(), "get_version returned null");
            CStr::from_ptr(ptr)
                .to_str()
                .expect("version is valid UTF-8")
                .to_owned()
        };
        let mut parts = version.split('.');
        assert!(
            parts.next().is_some_and(|p| p.parse::<u32>().is_ok()),
            "version {version:?} does not start with a number"
        );
    }

    // ---- Exercising the data-carrying ABI ------------------------------------
    //
    // `get_version` proves only that the library is linked. These tests drive
    // `compute_diff`, which is the part that can be subtly wrong: struct field
    // order, array marshalling, following `inner_changes`, and freeing.

    /// Lines marshalled the way the C expects: an array of pointers to
    /// NUL-terminated strings.
    ///
    /// A Rust `&str` is *not* NUL-terminated and the C calls `strlen`, so
    /// handing over `s.as_ptr()` would read past the end. The `CString`s must
    /// outlive the pointer array, which is why both live in one struct.
    struct Lines {
        _owned: Vec<CString>,
        ptrs: Vec<*const c_char>,
    }

    impl Lines {
        fn new(lines: &[&str]) -> Self {
            let owned: Vec<CString> = lines
                .iter()
                .map(|line| CString::new(*line).expect("test input has no interior NUL"))
                .collect();
            let ptrs = owned.iter().map(|s| s.as_ptr()).collect();
            Self {
                _owned: owned,
                ptrs,
            }
        }
    }

    #[derive(Debug, PartialEq, Eq)]
    struct DetailedLineRangeMapping {
        original: LineRange,
        modified: LineRange,
        inner_changes: Vec<RangeMapping>,
    }

    #[derive(Debug)]
    struct Snapshot {
        changes: Vec<DetailedLineRangeMapping>,
        moves: Vec<MovedText>,
        hit_timeout: bool,
    }

    /// Computes a diff, copies the result into owned Rust values, and frees the
    /// C allocation before returning. This is the shape the safe wrapper takes
    /// in S2: nothing owned by C outlives the call.
    fn diff_with(original: &[&str], modified: &[&str], options: DiffOptions) -> Snapshot {
        let orig = Lines::new(original);
        let modi = Lines::new(modified);

        let raw = unsafe {
            compute_diff(
                orig.ptrs.as_ptr(),
                orig.ptrs.len() as c_int,
                modi.ptrs.as_ptr(),
                modi.ptrs.len() as c_int,
                &options,
            )
        };
        assert!(!raw.is_null(), "compute_diff returned null");

        let snapshot = unsafe {
            let d = &*raw;
            let changes = (0..d.changes.count as isize)
                .map(|i| {
                    let m = &*d.changes.mappings.offset(i);
                    let inner_changes = if m.inner_changes.is_null() {
                        assert_eq!(m.inner_change_count, 0, "null inner_changes with a count");
                        Vec::new()
                    } else {
                        (0..m.inner_change_count as isize)
                            .map(|j| *m.inner_changes.offset(j))
                            .collect()
                    };
                    DetailedLineRangeMapping {
                        original: m.original,
                        modified: m.modified,
                        inner_changes,
                    }
                })
                .collect();
            let moves = (0..d.moves.count as isize)
                .map(|i| *d.moves.moves.offset(i))
                .collect();
            Snapshot {
                changes,
                moves,
                hit_timeout: d.hit_timeout,
            }
        };

        unsafe { free_lines_diff(raw) };
        snapshot
    }

    fn options() -> DiffOptions {
        DiffOptions {
            ignore_trim_whitespace: false,
            max_computation_time_ms: 5_000,
            compute_moves: false,
            extend_to_subwords: false,
        }
    }

    fn diff(original: &[&str], modified: &[&str]) -> Snapshot {
        diff_with(original, modified, options())
    }

    #[test]
    fn identical_input_produces_no_changes() {
        let d = diff(&["alpha", "beta", "gamma"], &["alpha", "beta", "gamma"]);
        assert!(d.changes.is_empty(), "unexpected changes: {:?}", d.changes);
        assert!(!d.hit_timeout);
    }

    #[test]
    fn a_modified_line_is_reported_with_1_based_exclusive_ranges() {
        let d = diff(&["alpha", "beta", "gamma"], &["alpha", "BETA", "gamma"]);
        assert_eq!(d.changes.len(), 1, "{:?}", d.changes);

        // Line 2 changed. Ranges are 1-indexed with an exclusive end, so the
        // second line alone is [2, 3).
        assert_eq!(
            d.changes[0].original,
            LineRange {
                start_line: 2,
                end_line: 3
            }
        );
        assert_eq!(
            d.changes[0].modified,
            LineRange {
                start_line: 2,
                end_line: 3
            }
        );
        assert!(
            !d.changes[0].inner_changes.is_empty(),
            "a modified line should carry character-level detail"
        );
    }

    #[test]
    fn an_inserted_line_leaves_the_original_range_empty() {
        let d = diff(&["alpha", "gamma"], &["alpha", "beta", "gamma"]);
        assert_eq!(d.changes.len(), 1, "{:?}", d.changes);

        let original = d.changes[0].original;
        assert_eq!(
            original.start_line, original.end_line,
            "an insertion touches no original line, so its range is empty"
        );
        assert_eq!(
            d.changes[0].modified,
            LineRange {
                start_line: 2,
                end_line: 3
            }
        );
    }

    #[test]
    fn a_deleted_line_leaves_the_modified_range_empty() {
        let d = diff(&["alpha", "beta", "gamma"], &["alpha", "gamma"]);
        assert_eq!(d.changes.len(), 1, "{:?}", d.changes);

        assert_eq!(
            d.changes[0].original,
            LineRange {
                start_line: 2,
                end_line: 3
            }
        );
        let modified = d.changes[0].modified;
        assert_eq!(
            modified.start_line, modified.end_line,
            "a deletion produces no modified line, so its range is empty"
        );
    }

    #[test]
    fn inner_changes_locate_the_edited_span_within_a_line() {
        // "one" -> "three" at columns 5..8 of line 1, 1-indexed, end exclusive.
        let d = diff(&["value one here"], &["value three here"]);
        assert_eq!(d.changes.len(), 1, "{:?}", d.changes);

        let inner = &d.changes[0].inner_changes;
        assert!(!inner.is_empty(), "expected character-level detail");

        let first = inner[0];
        assert_eq!(first.original.start_line, 1);
        assert_eq!(first.modified.start_line, 1);
        assert!(
            first.original.start_col > 1,
            "the shared prefix \"value \" should not be part of the change, got {first:?}"
        );
        assert!(first.original.end_col > first.original.start_col);
    }

    #[test]
    fn multibyte_lines_survive_marshalling() {
        // The C indexes columns in UTF-16 code units, so a line containing
        // characters outside the BMP is the case most likely to expose a
        // marshalling or indexing mistake.
        let d = diff(&["日本語 alpha 🎉"], &["日本語 beta 🎉"]);
        assert_eq!(d.changes.len(), 1, "{:?}", d.changes);
        assert!(!d.changes[0].inner_changes.is_empty());
    }

    #[test]
    fn many_lines_are_marshalled_without_truncation() {
        // Guards the pointer-array plumbing: a length mistake would show up as
        // a change reported outside the input, or as a crash.
        let original: Vec<String> = (0..500).map(|i| format!("line {i}")).collect();
        let mut modified = original.clone();
        modified[250] = "line 250 edited".to_owned();

        let orig: Vec<&str> = original.iter().map(String::as_str).collect();
        let modi: Vec<&str> = modified.iter().map(String::as_str).collect();

        let d = diff(&orig, &modi);
        assert_eq!(d.changes.len(), 1, "{:?}", d.changes);
        assert_eq!(
            d.changes[0].original,
            LineRange {
                start_line: 251,
                end_line: 252
            }
        );
    }

    #[test]
    fn an_empty_file_is_one_empty_line_not_zero_lines() {
        // The C mirrors VSCode's document model, in which every document has at
        // least one line. `compute_diff` recognises `[""]` as an empty file and
        // takes a dedicated full-file path for it.
        let d = diff(&[""], &["alpha", "beta"]);
        assert_eq!(d.changes.len(), 1, "{:?}", d.changes);
        assert_eq!(
            d.changes[0].original,
            LineRange {
                start_line: 1,
                end_line: 2
            }
        );
        assert_eq!(
            d.changes[0].modified,
            LineRange {
                start_line: 1,
                end_line: 3
            }
        );
        assert!(!d.hit_timeout);
    }

    #[test]
    fn zero_lines_is_outside_the_contract_and_silently_reports_nothing() {
        // A count of 0 does not crash; it reports no changes at all, which is
        // wrong for a file whose entire content was added.
        //
        // Why: `arrays_equal` rejects the equality shortcut because the counts
        // differ, the full-file path requires `count == 1`, and the algorithm
        // is then handed an empty sequence.
        //
        // Pinned rather than fixed, because the C is upstream and unmodified.
        // The safe wrapper in S2 must normalise an empty side to `[""]` before
        // calling in, or an entirely-added file will silently appear unchanged.
        let d = diff(&[], &["alpha", "beta"]);
        assert!(
            d.changes.is_empty(),
            "upstream behaviour changed; revisit the S2 normalisation: {:?}",
            d.changes
        );
    }

    #[test]
    fn moves_are_reported_only_when_requested() {
        let original = [
            "aaa", "bbb", "ccc", "ddd", "eee", "fff", "block1", "block2", "block3",
        ];
        let modified = [
            "block1", "block2", "block3", "aaa", "bbb", "ccc", "ddd", "eee", "fff",
        ];

        let without = diff_with(&original, &modified, options());
        assert!(
            without.moves.is_empty(),
            "compute_moves was false, got {:?}",
            without.moves
        );

        let with = diff_with(
            &original,
            &modified,
            DiffOptions {
                compute_moves: true,
                ..options()
            },
        );
        assert!(
            !with.moves.is_empty(),
            "a relocated block should be reported as a move"
        );
    }

    #[test]
    fn repeated_calls_do_not_corrupt_state() {
        // Every call allocates and frees; running many in sequence would surface
        // a double free or a use-after-free as a crash under the test harness.
        for i in 0..200 {
            let modified = format!("beta {i}");
            let d = diff(&["alpha", "beta", "gamma"], &["alpha", &modified, "gamma"]);
            assert_eq!(d.changes.len(), 1);
        }
    }
}
