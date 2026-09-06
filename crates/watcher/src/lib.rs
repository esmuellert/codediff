//! Watches a Git repository and reports refresh categories.

pub mod filter;
mod git_dirs;
mod ignore_rules;
mod scope;
mod watch;

mod refresh;
pub use refresh::Refresh;
pub use watch::{Subscription, subscribe};
