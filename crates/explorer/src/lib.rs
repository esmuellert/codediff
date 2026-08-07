#![doc = include_str!("../README.md")]
//!
//! ---
//!
//! Admission criterion: does this decide *which files are listed and in what
//! shape*? Never how they are read, and never how they look:
//!
//! ```text
//! request.rs   what set of files to show, and where from
//! group.rs     files that share a comparison
//! entry.rs     one changed file, as one row will show it
//! node.rs      what the tree is made of
//! tree.rs      building it, and collapsing what has no choice in it
//! order.rs     what comes before what
//! filter.rs    hiding rows by a glob
//! rows.rs      walking the tree into visible lines
//! row.rs       one line, as facts — never as text
//! model.rs     the state, and everything that changes it
//! ```
//!
//! Groups are assembled outside: which comparisons exist, which files are in
//! each, and how many lines a file gained are questions only a backend can
//! answer, and `cargo xtask lint-arch` forbids this crate from asking one.

mod entry;
mod filter;
mod group;
mod model;
mod node;
mod order;
mod request;
mod row;
mod rows;
mod tree;

pub use entry::Entry;
pub use filter::matches;
pub use group::{Group, Groups};
pub use model::{Anchor, Explorer};
pub use node::{EntryId, Node, NodeId, NodeType};
pub use request::{ExplorerDiffRequest, ExplorerDiffType};
pub use row::{Content, Guides, Row};
pub use tree::ViewMode;
