//! A safe wrapper over the libvscode-diff C engine.
//!
//! ```
//! use vscode_diff::{Options, compute};
//!
//! let original = ["fn main() {", "    let x = 1;", "}"];
//! let modified = ["fn main() {", "    let x = 42;", "    dbg!(x);", "}"];
//!
//! let diff = compute(&original, &modified, &Options::default())?;
//! assert!(!diff.is_empty());
//! # Ok::<(), vscode_diff::Error>(())
//! ```
//!
//! The boundary rule is *convert eagerly, free immediately*: C memory is walked
//! once into owned Rust values and released before returning, so a [`LinesDiff`] is
//! an ordinary value with no borrows and no hidden lifetime. All `unsafe` lives
//! in `vscode_diff_sys` and this crate's `convert` module.

mod convert;
mod error;
mod options;

use std::ffi::{CStr, CString};
use std::os::raw::c_char;

pub use error::{Error, Side};
pub use options::Options;
// Re-exported so a caller that already depends on this crate need not also name
// `diff-types`. The structs themselves live there, with no dependencies and no
// build script, so `align` can name a diff without inheriting a C toolchain.
pub use diff_types::{
    CharRange, DetailedLineRangeMapping, LineRange, LinesDiff, MovedText, RangeMapping,
};

/// The version of the C diff engine compiled into this binary.
///
/// This is the vendored engine's version, not this crate's.
pub fn engine_version() -> &'static str {
    // The engine returns a pointer to a static string literal, so it is
    // non-null and lives for the duration of the program.
    #[allow(unsafe_code)]
    let raw = unsafe { vscode_diff_sys::get_version() };

    debug_assert!(!raw.is_null(), "get_version must never return null");

    #[allow(unsafe_code)]
    let cstr = unsafe { CStr::from_ptr(raw) };

    cstr.to_str()
        .expect("the engine version is an ASCII string literal")
}

/// Splits text into the lines [`compute`] counts.
///
/// On `\n` only, keeping the empty piece a trailing newline leaves behind:
/// the engine counts that line, so both sides must agree it is there.
///
/// `str::lines()` is wrong here twice over. It drops that final piece, and it
/// swallows a `\r` before the newline — which would hide exactly the
/// line-ending differences a reviewer needs to see.
///
/// ```
/// assert_eq!(vscode_diff::lines("a\nb\n"), ["a", "b", ""]);
/// assert_eq!(vscode_diff::lines("a\r\nb"), ["a\r", "b"]);
/// // An empty file is one empty line, not none — see `compute`.
/// assert_eq!(vscode_diff::lines(""), [""]);
/// ```
pub fn lines(text: &str) -> Vec<&str> {
    text.split('\n').collect()
}

/// Computes the difference between two texts, given as lines without their
/// terminators.
///
/// Line numbers in the result are 1-based and ranges are end-exclusive;
/// columns are 1-based and counted in UTF-16 code units.
///
/// # Empty input
///
/// An empty side may be given as either `&[]` or `&[""]`; both are normalised
/// to the engine's representation of an empty file, which is a single empty
/// line. This matters: passing a zero-length array straight through would make
/// the engine report *no changes at all* for a file whose entire content was
/// added.
///
/// # Errors
///
/// Returns [`Error::InteriorNul`] if a line contains a NUL byte, which the
/// engine's NUL-terminated strings cannot represent, and [`Error::OutOfMemory`]
/// if the engine could not allocate its result.
pub fn compute(
    original: &[&str],
    modified: &[&str],
    options: &Options,
) -> Result<LinesDiff, Error> {
    let original = Marshalled::new(original, Side::Original)?;
    let modified = Marshalled::new(modified, Side::Modified)?;
    let raw_options = vscode_diff_sys::DiffOptions::from(*options);

    // SAFETY: both pointer arrays are non-empty, live for the duration of this
    // call, and point to NUL-terminated strings owned by the `Marshalled`
    // values; `raw_options` is a live local.
    #[allow(unsafe_code)]
    let raw = unsafe {
        vscode_diff_sys::compute_diff(
            original.as_ptr(),
            original.len(),
            modified.as_ptr(),
            modified.len(),
            &raw_options,
        )
    };

    if raw.is_null() {
        return Err(Error::OutOfMemory);
    }

    // SAFETY: `raw` is non-null and came from `compute_diff` immediately above.
    // `take` frees it, and it is not used afterwards.
    #[allow(unsafe_code)]
    let diff = unsafe { convert::take(raw) };

    Ok(diff)
}

/// Lines converted to the array of NUL-terminated pointers the engine expects.
///
/// The `CString`s must outlive the pointer array, so both are held together.
/// A Rust `&str` is not NUL-terminated and the engine calls `strlen`, so
/// passing `str::as_ptr` directly would read past the end of the string.
struct Marshalled {
    _owned: Vec<CString>,
    pointers: Vec<*const c_char>,
}

impl Marshalled {
    fn new(lines: &[&str], side: Side) -> Result<Self, Error> {
        // The engine models an empty file as one empty line, following
        // VSCode's document model. A count of zero is not equivalent: it
        // silently yields no changes.
        let empty = [""];
        let lines = if lines.is_empty() { &empty[..] } else { lines };

        let mut owned = Vec::with_capacity(lines.len());
        for (index, line) in lines.iter().enumerate() {
            let cstring = CString::new(*line).map_err(|_| Error::InteriorNul {
                side,
                line: index + 1,
            })?;
            owned.push(cstring);
        }

        let pointers = owned.iter().map(|line| line.as_ptr()).collect();
        Ok(Self {
            _owned: owned,
            pointers,
        })
    }

    fn as_ptr(&self) -> *const *const c_char {
        self.pointers.as_ptr()
    }

    fn len(&self) -> i32 {
        i32::try_from(self.pointers.len()).unwrap_or(i32::MAX)
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn a_trailing_newline_leaves_an_empty_last_line() {
        // The engine counts it, so both sides must agree that it is there.
        assert_eq!(super::lines("a\nb\n"), ["a", "b", ""]);
        assert_eq!(super::lines("a\nb"), ["a", "b"]);
    }

    #[test]
    fn empty_text_is_one_empty_line() {
        // Not zero lines: `compute` documents that the engine models an empty
        // file as a single empty line, and reports nothing at all for a
        // genuinely empty sequence.
        assert_eq!(super::lines(""), [""]);
    }

    #[test]
    fn carriage_returns_are_kept() {
        // `str::lines()` would eat these, and a file that gained CRLF endings
        // would then diff as unchanged.
        assert_eq!(super::lines("a\r\nb\r\n"), ["a\r", "b\r", ""]);
    }

    use super::*;

    #[test]
    fn reports_a_dotted_engine_version() {
        let version = engine_version();
        assert!(
            version.split('.').take(3).all(|p| p.parse::<u32>().is_ok()),
            "expected MAJOR.MINOR.PATCH, got {version:?}"
        );
    }

    #[test]
    fn an_interior_nul_is_reported_rather_than_truncating() {
        let err = compute(&["fine"], &["bro\0ken"], &Options::default()).unwrap_err();
        assert_eq!(
            err,
            Error::InteriorNul {
                side: Side::Modified,
                line: 1
            }
        );
    }

    #[test]
    fn both_spellings_of_empty_behave_identically() {
        let from_none = compute(&[], &["alpha"], &Options::default()).unwrap();
        let from_blank = compute(&[""], &["alpha"], &Options::default()).unwrap();
        assert_eq!(from_none, from_blank);
        assert!(
            !from_none.is_empty(),
            "adding content to an empty file is a change"
        );
    }
}
