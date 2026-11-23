#![doc = include_str!("../README.md")]

mod content;
mod file;
mod oid;
mod path;
mod rev;
mod stats;
mod version;
mod viewer_mode;

pub use content::FileContent;
pub use file::{ChangeType, File, Nowhere, Revs};
pub use oid::Oid;
pub use path::RepoPath;
pub use rev::{Rev, Stage};
pub use stats::Stats;
pub use version::DiffVersion;
pub use viewer_mode::ViewerMode;
