//! Wires `vcs`, `vscode-diff` and `align` into pipelines for the interface.
//!
//! Two pipelines:
//! - `files`: a repository request → `Vec<file_types::File>`
//! - `diff`: a file → renderable diff content

pub mod diff;
pub mod files;
