//! From a question to something the interface can draw.
//!
//! ---
//!
//! Admission criterion: is this a step between a request and an answer the
//! interface can hold? Two pipelines of the same shape, one folder each:
//!
//! | | | in | out |
//! |---|---|---|---|
//! | [`list`] | a set of files | a [`Request`](list::Request) | `Vec<`[`File`]`>` |
//! | [`file`] | one of them, in four | a [`File`] | a [`DiffContent`] |
//!
//! [`File`]: file_types::File
//! [`DiffContent`]: file::DiffContent
//!
//! **The list's item type is the file's input type.** That is what makes them
//! two pipelines rather than one thing bolted to another, and it is the whole
//! join: the reader picks a row, and the row is already the next request.
//!
//! ```text
//! list ──▶ File ──▶ (a row) ──▶ file ──▶ DiffContent
//! ```
//!
//! The file pipeline used to search for a file by path, which was the list
//! written again — and worse, since a search cannot know which comparison the
//! reader chose. It answered `HEAD → worktree` for everything, so one path had
//! three different diffs depending on how it was reached. See D58.
//!
//! **This crate must never name `ui`.** It is the only one that names `vcs`,
//! `vscode-diff` and `align` together, and `ui` depends on it —
//! the interface owns the threads that run these, because it owns the loop
//! that collects from them. An answer therefore stops at [`DiffContent`], which
//! is data; deciding what to *draw* from it is the interface's, and pointing
//! that arrow back would be a cycle.

pub mod file;
pub mod list;
