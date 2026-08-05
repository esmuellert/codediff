#![doc = include_str!("../README.md")]
//!
//! ---
//!
//! Admission criterion: is this part of *what a file under review is*? Never
//! how one is read, diffed, or drawn — those are `vcs`, `align` and `ui`, all
//! of which depend on this and none of which it can see.
//!
//! Four types, no dependencies, no build script. Every layer names this crate,
//! which is the whole point: a file's identity is converted at no boundary,
//! and so cannot degrade at one.

mod changed;
mod content;
mod file;
mod oid;
mod path;
mod rev;
mod version;

pub use changed::ChangedFile;
pub use content::FileContent;
pub use file::{ChangeType, File, Nowhere, Revs};
pub use oid::Oid;
pub use path::RepoPath;
pub use rev::{Rev, Stage};
pub use version::DiffVersion;
