//! Failures that can occur while computing a diff.

use std::fmt;

use file_types::DiffVersion;

/// Why a diff could not be computed.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Error {
    /// A line contained a NUL byte.
    ///
    /// The engine takes NUL-terminated C strings, so such a line cannot be
    /// passed through faithfully. Source files do not contain NUL bytes;
    /// binary content does, and should be detected before reaching here.
    InteriorNul { version: DiffVersion, line: usize },

    /// The engine could not allocate its result.
    OutOfMemory,
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InteriorNul { version, line } => write!(
                f,
                "{} line {line} contains a NUL byte, which the diff engine cannot represent",
                name(*version)
            ),
            Self::OutOfMemory => f.write_str("the diff engine could not allocate its result"),
        }
    }
}

impl std::error::Error for Error {}

/// What to call a version in a message.
///
/// `DiffVersion` has no `Display`: it is a selector, and how to
/// spell it is the caller's business — a status line might say "before".
fn name(version: DiffVersion) -> &'static str {
    match version {
        DiffVersion::Original => "original",
        DiffVersion::Modified => "modified",
    }
}
