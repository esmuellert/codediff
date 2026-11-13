//! Lists changed files and diffs and colours them on background threads.
//!
//! `Pipeline::list_files` and `Pipeline::open` return shared slots that the
//! threads fill in before asking the interface to redraw.

pub mod diff;
pub mod files;
mod threads;

pub use threads::Pipeline;
