#![doc = include_str!("../README.md")]

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
