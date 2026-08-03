//! Reading the working tree — the checkout on disk.
//!
//! Not a git command: it is `std::fs`. It lives here because the working tree
//! is one of the two sides of the default comparison, and belongs behind the
//! same interface as the side that does come from the object store.

use crate::error::{Error, Result};
use file_types::RepoPath;

/// A file's current content. `None` when it is not on disk — a deletion, or a
/// path that only exists in the revision being compared against.
///
/// Takes no root: a [`RepoPath`] already carries its absolute form, which is
/// the reason it carries one. Passing the two separately is how they come to
/// disagree.
pub fn read(path: &RepoPath) -> Result<Option<Vec<u8>>> {
    match std::fs::read(path.as_path()) {
        Ok(bytes) => Ok(Some(bytes)),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(source) => Err(Error::Io {
            path: path.as_path().to_path_buf(),
            source,
        }),
    }
}
