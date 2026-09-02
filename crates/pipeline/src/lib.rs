//! Wires `vcs`, `vscode-diff` and `align` into pipelines for the interface.
//!
//! Two pipelines:
//! - [`list`]: a request → `Vec<File>`
//! - [`file`]: a `File` → `DiffContent`
//!
//! The list's output type is the file pipeline's input type.
//!
//! [`File`]: file_types::File
//! [`DiffContent`]: file::DiffContent

pub mod diff;
pub mod files;
