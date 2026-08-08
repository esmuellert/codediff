#![doc = include_str!("../README.md")]
//!
//! ---
//!
//! Admission criterion: is this part of *what a file under review is*? Never
//! how one is read or diffed — those are `vcs` and `align`, both of which
//! depend on this and neither of which it can see.
//!
//! **Including how it is shown**, where that is a fact every layer must agree
//! on. [`DiffType`] is here because the pipeline that produces a file, the
//! pairing that describes it and the interface that draws it were spelling one
//! fork four different ways, each of them an `Option` whose `None` meant
//! "single file". A word shared by layers that cannot see each other is
//! exactly what this crate is for.
//!
//! No dependencies, no build script. Every layer names this crate, which is
//! the whole point: a file's identity is converted at no boundary, and so
//! cannot degrade at one.

mod content;
mod diff_type;
mod file;
mod oid;
mod path;
mod rev;
mod stats;
mod version;

pub use content::FileContent;
pub use diff_type::DiffType;
pub use file::{ChangeType, File, Nowhere, Revs};
pub use oid::Oid;
pub use path::RepoPath;
pub use rev::{Rev, Stage};
pub use stats::Stats;
pub use version::DiffVersion;
