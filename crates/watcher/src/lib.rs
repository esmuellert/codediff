//! Watches a git repository for changes and reports what needs refreshing.
//!
//! ```text
//! lib.rs       re-exports
//! refresh.rs   Refresh — the bitset of what changed
//! filter.rs    path → Refresh (pure logic, all filtering)
//! scope.rs     computes and maintains the paths handed to notify
//! watch.rs     the debouncer, the thread, the handle
//! ```

pub mod filter;
mod scope;
mod watch;

mod refresh;
pub use refresh::Refresh;
pub use watch::{Subscription, subscribe};
