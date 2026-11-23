#![doc = include_str!("../README.md")]
//!
//! ```text
//! repository/   open, list changes, commits, count, read
//! repo.rs       where a repository is
//! error.rs      how running one can fail
//! git/          private — one file per command
//! ```

mod error;
mod git;
mod repo;
mod repository;

pub use error::{Error, Result};
pub use repo::Repo;
pub use repository::{Commit, CommitOrder, Repository, RevisionRange};
