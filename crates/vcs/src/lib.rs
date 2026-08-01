#![doc = include_str!("../README.md")]
//!
//! ---
//!
//! Admission criterion: which **capability** does this belong to? Every trait
//! gets a folder holding itself and the types in its signatures, so adding one
//! means adding a folder rather than growing a file. A crate named for a whole
//! domain is otherwise an invitation to put anything in it.
//!
//! `path`, `repo` and `error` sit above the capabilities because all of them
//! need those. `git` sits below, and is the only place `git` runs.

mod error;
mod path;
mod repo;

pub mod diff;
pub mod git;

pub use diff::{Diff, DiffKind, FileDiff};
pub use error::{Error, Result};
pub use git::Git;
pub use path::RelPath;
pub use repo::Repo;
