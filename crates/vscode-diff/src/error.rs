//! Failures that can occur while computing a diff.

use std::fmt;

use file_types::DiffVersion;

/// Why a diff could not be computed.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Error {
    /// A line contains a NUL byte and cannot be passed to the C engine.
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

/// Returns the display name for a file version.
fn name(version: DiffVersion) -> &'static str {
    match version {
        DiffVersion::Original => "original",
        DiffVersion::Modified => "modified",
    }
}
