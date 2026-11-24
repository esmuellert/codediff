//! Watches a repository and multicasts refresh events to UI subscribers.

use std::cell::RefCell;
use std::path::Path;

use loom::{Observer, observable};
use watcher::{Refresh, Subscription};

/// Owns the filesystem watcher subscription and multicasts [`Refresh`] events.
pub struct WatcherService {
    _subscription: Option<Subscription>,
    observers: RefCell<Vec<Observer<Refresh>>>,
}

impl WatcherService {
    /// Starts watching `repo` and forwarding events through `emitter`.
    pub fn start(repo: &Path, emitter: channel::Emitter<Refresh>) -> anyhow::Result<Self> {
        let subscription = watcher::subscribe(repo, emitter)?;
        Ok(Self {
            _subscription: Some(subscription),
            observers: RefCell::new(Vec::new()),
        })
    }

    /// Creates an in-memory watcher service without a filesystem watcher for tests.
    pub fn fake() -> Self {
        Self {
            _subscription: None,
            observers: RefCell::new(Vec::new()),
        }
    }

    /// Subscribes `callback` to repository refresh notifications.
    pub fn subscribe(&self, callback: impl FnMut(Refresh) + 'static) {
        let (observer, changes) = observable();
        changes.subscribe(callback);
        let mut observers = self.observers.borrow_mut();
        observers.retain(Observer::is_wanted);
        observers.push(observer);
    }

    /// Delivers `refresh` to every active subscriber.
    pub fn notify(&self, refresh: Refresh) {
        let mut observers = self.observers.borrow_mut();
        observers.retain(Observer::is_wanted);
        for observer in observers.iter() {
            observer.next(refresh);
        }
    }
}
