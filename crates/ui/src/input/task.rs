//! What has to leave the crate, and the keys that ask for it.
//!
//! Not a level of the view model, and not executed here at all: a task is a
//! **request**, named by `ui` and performed by the composition root. That
//! is the only way opening a file can exist here without `ui` depending
//! on `vcs`, which `cargo xtask lint-arch` forbids.
//!
//! Uninhabited. After startup this binary performs no IO, so there is nothing
//! a key could ask for yet; the first will be the explorer opening a file.

/// Leaves the crate. May fail, may take milliseconds.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TaskAction {}
