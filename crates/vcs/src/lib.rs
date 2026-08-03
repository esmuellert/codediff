#![doc = include_str!("../README.md")]
//!
//! ---
//!
//! Admission criterion: does this **run a version control system**, or say how
//! doing so can fail? What it *produces* is `file-types` — `ChangedFile`,
//! `File`, `RepoPath`, `FileContent` — and `cargo xtask lint-arch` forbids
//! that crate from naming this one, so no git concept can reach a reviewer.
//!
//! There is no trait. The contract is the types, and the pipeline that calls
//! `Git`'s methods is what checks a backend meets it. A second backend earns a
//! trait extracted from two real implementations; one guessed from a single
//! implementor was checking nothing. See D30.
//!
//! `repo` and `error` sit above; `git` sits below and is the only place `git`
//! runs.

mod error;
mod repo;

pub mod git;

pub use error::{Error, Result};
pub use git::Git;
pub use repo::Repo;
