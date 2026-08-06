//! From a question to something the interface can draw.
//!
//! ---
//!
//! Admission criterion: is this a step between a request and an answer the
//! interface can hold? Two pipelines of the same shape, one folder each:
//!
//! | | | in | out |
//! |---|---|---|---|
//! | [`list`] | a set of files, in two stages | an [`ExplorerDiffRequest`] | [`Groups`] |
//! | [`file`] | one of them, in four | a [`ChangedFile`] | a [`Buffer`] |
//!
//! [`Buffer`]: ui::Buffer
//! [`ExplorerDiffRequest`]: explorer::ExplorerDiffRequest
//! [`Groups`]: explorer::Groups
//!
//! **The list's item type is the file's input type.** That is what makes them
//! two pipelines rather than one thing bolted to another, and it is the whole
//! join: the reader picks a row, and the row is already the next request.
//!
//! ```text
//! list ──▶ Groups ──▶ (a row) ──▶ file ──▶ Buffer
//! ```
//!
//! The file pipeline used to search for a file by path, which was the list
//! written again — and worse, since a search cannot know which comparison the
//! reader chose. It answered `HEAD → worktree` for everything, so one path had
//! three different diffs depending on how it was reached. See D58.
//!
//! This lives in the binary because it is the only crate allowed to name
//! `vcs`, `vscode-diff`, `align`, `explorer` and `ui` together — `cargo xtask
//! lint-arch` forbids those edges everywhere else. A renderer that could
//! assemble its own input would be a renderer that can shell out to git, which
//! is the failure that produced a 674-line `explorer/render.lua` in the plugin.

pub mod file;
pub mod list;
