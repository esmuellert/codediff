//! Bridges between components and worker threads.

pub mod diff;
pub mod syntax;
pub mod vcs;
pub mod watcher;

pub use self::diff::DiffService;
pub use self::syntax::SyntaxService;
pub use self::vcs::VcsService;
pub use self::watcher::WatcherService;
