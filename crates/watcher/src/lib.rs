//! Watches a git repository for changes and reports what needs refreshing.
//!
//! ```text
//! lib.rs       re-exports
//! refresh.rs   Refresh — the bitset of what changed
//! filter.rs         path → Refresh (pure logic, all filtering)
//! git_dirs.rs       resolves worktree-specific and common Git directories
//! ignore_rules.rs   loads and detects changes to ignore rules
//! scope.rs          computes and maintains the paths handed to notify
//! watch.rs          the bounded debouncer, the thread, the handle
//! ```

pub mod filter;
mod git_dirs;
mod ignore_rules;
mod scope;
mod watch;

mod refresh;
pub use refresh::Refresh;
pub use watch::{Subscription, subscribe};
