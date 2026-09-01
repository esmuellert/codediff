//! Safe bindings for the libvscode-diff C engine.

mod convert;
mod error;
mod options;

use std::ffi::{CStr, CString};
use std::os::raw::c_char;

pub use diff_types::{
    CharRange, DetailedLineRangeMapping, LineRange, LinesDiff, MovedText, RangeMapping,
};
pub use error::Error;
pub use file_types::DiffVersion;
pub use options::Options;

/// The compiled C engine's version.
pub fn engine_version() -> &'static str {
    // SAFETY: the engine returns a static, non-null string.
    #[allow(unsafe_code)]
    let raw = unsafe { vscode_diff_sys::get_version() };

    debug_assert!(!raw.is_null(), "get_version must never return null");

    #[allow(unsafe_code)]
    let cstr = unsafe { CStr::from_ptr(raw) };

    cstr.to_str()
        .expect("the engine version is an ASCII string literal")
}

/// Splits on `\n`, retaining carriage returns and a trailing empty line.
pub fn lines(text: &str) -> Vec<&str> {
    text.split('\n').collect()
}

/// Splits CRLF, bare CR, and LF as VS Code's text model does.
pub fn editor_lines(text: &str) -> Vec<&str> {
    let bytes = text.as_bytes();
    let mut lines = Vec::new();
    let mut start = 0;
    let mut at = 0;
    while at < bytes.len() {
        match bytes[at] {
            b'\r' => {
                lines.push(&text[start..at]);
                at += usize::from(bytes.get(at + 1) == Some(&b'\n')) + 1;
                start = at;
            }
            b'\n' => {
                lines.push(&text[start..at]);
                at += 1;
                start = at;
            }
            _ => at += 1,
        }
    }
    lines.push(&text[start..]);
    lines
}

/// Computes a diff with 1-based lines and UTF-16 columns.
///
/// Empty input is treated as one empty line.
pub fn compute(
    original: &[&str],
    modified: &[&str],
    options: &Options,
) -> Result<LinesDiff, Error> {
    let original = Marshalled::new(original, DiffVersion::Original)?;
    let modified = Marshalled::new(modified, DiffVersion::Modified)?;
    let raw_options = vscode_diff_sys::DiffOptions::from(*options);

    // SAFETY: `Marshalled` owns both non-empty pointer arrays for this call.
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

    // SAFETY: `raw` is the live result above; `take` frees it.
    #[allow(unsafe_code)]
    let diff = unsafe { convert::take(raw) };

    Ok(diff)
}

/// Owns the C strings behind an engine pointer array.
struct Marshalled {
    _owned: Vec<CString>,
    pointers: Vec<*const c_char>,
}

impl Marshalled {
    fn new(lines: &[&str], version: DiffVersion) -> Result<Self, Error> {
        let empty = [""];
        let lines = if lines.is_empty() { &empty[..] } else { lines };

        let mut owned = Vec::with_capacity(lines.len());
        for (index, line) in lines.iter().enumerate() {
            let cstring = CString::new(*line).map_err(|_| Error::InteriorNul {
                version,
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
        assert_eq!(super::lines("a\nb\n"), ["a", "b", ""]);
        assert_eq!(super::lines("a\nb"), ["a", "b"]);
    }

    #[test]
    fn empty_text_is_one_empty_line() {
        assert_eq!(super::lines(""), [""]);
    }

    #[test]
    fn carriage_returns_are_kept() {
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
                version: DiffVersion::Modified,
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
