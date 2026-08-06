//! What has to leave the crate, and the keys that ask for it.
//!
//! Not a level of the view model, and not executed here at all: a task is a
//! **request**, named by `ui` and performed by the composition root. That
//! is the only way opening a file can exist here without `ui` depending
//! on `vcs`, which `cargo xtask lint-arch` forbids.
//!
//! One so far. It leaves through [`Flow`](crate::app::Flow) rather than being
//! run, so nothing in this crate needs a repository, a thread, or an error
//! type that can say "git is not installed".

/// Leaves the crate. May fail, may take milliseconds.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TaskAction {
    /// Open what the reader has selected in the list.
    ///
    /// The row may turn out to be a directory, in which case the buffer folds
    /// it and nothing leaves the crate. Which of the two it is, is the
    /// buffer's answer and not the key's: one key does the obvious thing on
    /// every row, exactly as it does in the plugin.
    Open,
}
