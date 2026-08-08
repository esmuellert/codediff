//! One request for a set of changed files.
//!
//! ```ignore
//! let files = list::get_files(&list::Request::worktree(root))?;
//! ```

pub mod entries;
mod request;

pub use request::Request;

use anyhow::Result;
use file_types::File;

/// Runs the request and hands over every file it found.
pub fn get_files(request: &Request) -> Result<Vec<File>> {
    entries::read(request)
}
