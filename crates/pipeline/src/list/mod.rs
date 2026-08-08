//! One request for a set of changed files.
//!
//! ```ignore
//! let files = list::run(&list::Request::worktree(root))?;
//! ```

pub mod entries;
mod request;

pub use request::Request;

use anyhow::Result;
use file_types::File;

/// Runs the request and hands over every file it found.
pub fn run(request: &Request) -> Result<Vec<File>> {
    entries::read(request)
}
