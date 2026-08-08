#![doc = include_str!("../README.md")]
//!
//! ---
//!
//! Admission criterion: does this **run a version control system**, or say how
//! doing so can fail? What it *produces* is `file-types` — `File`,
//! `File`, `RepoPath`, `FileContent` — and `cargo xtask lint-arch` forbids
//! that crate from naming this one, so no git concept can reach a reviewer.
//!
//! ```text
//! repository/   the whole surface: open, changes, counts, read
//! repo.rs       where a repository is
//! error.rs      how running one can fail
//! git/          PRIVATE — the backend, one file per command
//! ```
//!
//! **`git` is private**, so nothing outside can run a git command, name a
//! status code, or hold a `--cached`. A second backend is a directory beside
//! it and an arm in [`Repository::open`] — not a search for every caller that
//! reached past the layer. See D67.

mod error;
mod git;
mod repo;
mod repository;

pub use error::{Error, Result};
pub use git::diff::numstat::Counts;
pub use repo::Repo;
pub use repository::{DiffType, Repository};
