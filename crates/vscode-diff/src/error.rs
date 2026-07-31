//! Failures that can occur while computing a diff.

use std::fmt;

/// Why a diff could not be computed.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Error {
    /// A line contained a NUL byte.
    ///
    /// The engine takes NUL-terminated C strings, so such a line cannot be
    /// passed through faithfully. Source files do not contain NUL bytes;
    /// binary content does, and should be detected before reaching here.
    InteriorNul { side: Side, line: usize },

    /// The engine could not allocate its result.
    OutOfMemory,
}

/// Which of the two inputs a failure refers to.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Side {
    Original,
    Modified,
}

impl fmt::Display for Side {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Original => f.write_str("original"),
            Self::Modified => f.write_str("modified"),
        }
    }
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InteriorNul { side, line } => write!(
                f,
                "{side} line {line} contains a NUL byte, which the diff engine cannot represent"
            ),
            Self::OutOfMemory => f.write_str("the diff engine could not allocate its result"),
        }
    }
}

impl std::error::Error for Error {}
