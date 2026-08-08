//! Four stages for one file: read both sides → diff → align → hand over.
//!
//! All four run on a worker thread (see [`service`]).
//!
//! ```ignore
//! let mut files = file::Files::start();
//! files.want(&changed);          // returns at once
//! let response = files.take();   // next frame, or the one after
//! ```
//!
pub mod contents;
pub mod diff;
pub mod runner;
pub mod service;

pub use runner::{Diff, DiffContent, Runner, SingleFile};
pub use service::{Files, Response};
