//! Bridges between components and worker threads.

pub mod diff;
pub mod syntax;
pub mod vcs;
pub mod watcher;

use std::sync::{Arc, Mutex, Weak};

pub use self::diff::DiffService;
pub use self::syntax::SyntaxService;
pub use self::vcs::VcsService;
pub use self::watcher::WatcherService;

/// An asynchronous result populated on a background service thread.
pub struct Resource<T>(Arc<Mutex<Option<Result<T, String>>>>);

impl<T> Clone for Resource<T> {
    fn clone(&self) -> Self {
        Self(Arc::clone(&self.0))
    }
}

impl<T> Resource<T> {
    pub(super) fn ready(value: Result<T, String>) -> Self {
        Self(Arc::new(Mutex::new(Some(value))))
    }

    pub(super) fn pending() -> (Self, Writer<T>) {
        let inner = Arc::new(Mutex::new(None));
        let writer = Writer(Arc::downgrade(&inner));
        (Self(inner), writer)
    }

    pub(super) fn writer(&self) -> Writer<T> {
        Writer(Arc::downgrade(&self.0))
    }

    pub fn is_ready(&self) -> bool {
        self.0.lock().unwrap().is_some()
    }

    pub fn take(&self) -> Option<Result<T, String>> {
        self.0.lock().unwrap().take()
    }

    pub fn get(&self) -> Option<Result<T, String>>
    where
        T: Clone,
    {
        self.0.lock().unwrap().clone()
    }
}

pub(super) struct Writer<T>(Weak<Mutex<Option<Result<T, String>>>>);

impl<T> Writer<T> {
    pub(super) fn fill(self, compute: impl FnOnce() -> Result<T, String>) {
        if self.0.strong_count() > 0 {
            let value = compute();
            if let Some(target) = self.0.upgrade() {
                *target.lock().unwrap() = Some(value);
            }
        }
    }

    pub(super) fn update(
        self,
        compute: impl FnOnce() -> Result<T, String>,
        unchanged: impl FnOnce(&T, &T) -> bool,
    ) {
        if self.0.strong_count() > 0 {
            let value = compute();
            if let Some(target) = self.0.upgrade() {
                let mut current = target.lock().unwrap();
                if let (Some(Ok(old)), Ok(new)) = (current.as_ref(), &value)
                    && unchanged(old, new)
                {
                    return;
                }
                *current = Some(value);
            }
        }
    }
}
