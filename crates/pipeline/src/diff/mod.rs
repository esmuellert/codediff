//! Four stages for one file: read both sides → diff → align → hand over.
//!
//! All four run on a worker thread (see [`service`]).
//!
//! ```ignore
//! let mut files = diff::DiffWorker::start();
//! files.want(&changed);          // returns at once
//! let response = files.poll();   // next frame, or the one after
//! ```
//!
pub mod contents;
pub mod diff;
pub mod runner;
pub mod worker;

pub use runner::{Diff, DiffContent, Runner, SingleFile};
pub use worker::{DiffWorker, Response};
