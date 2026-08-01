//! Reading the working tree — the checkout on disk.
//!
//! Not a git command: it is `std::fs`. It lives here because the working tree
//! is one of the two sides of the default comparison, and belongs behind the
//! same interface as the side that does come from the object store.

use std::path::Path;

use crate::error::{Error, Result};
use crate::path::RelPath;

/// A file's current content. `None` when it is not on disk — a deletion, or a
/// path that only exists in the revision being compared against.
pub fn read(root: &Path, path: &RelPath) -> Result<Option<Vec<u8>>> {
    match std::fs::read(path.to_absolute(root)) {
        Ok(bytes) => Ok(Some(bytes)),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(source) => Err(Error::Io {
            path: path.to_absolute(root),
            source,
        }),
    }
}
