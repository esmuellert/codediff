//! The bridge between repository watcher events and their consumers.

use std::cell::RefCell;

use loom::{Observable, Observer, observable};
use watcher::Refresh;

/// A multicast stream of repository changes.
pub struct WatcherService {
    observers: RefCell<Vec<Observer<Refresh>>>,
}

impl WatcherService {
    pub fn new() -> Self {
        Self {
            observers: RefCell::new(Vec::new()),
        }
    }

    pub fn changes(&self) -> Observable<Refresh> {
        let (observer, changes) = observable();
        let mut observers = self.observers.borrow_mut();
        observers.retain(Observer::is_wanted);
        observers.push(observer);
        changes
    }

    /// The loop calls this when the watcher reports a change.
    pub fn deliver(&self, refresh: Refresh) {
        let mut observers = self.observers.borrow_mut();
        observers.retain(Observer::is_wanted);
        for observer in observers.iter() {
            observer.next(refresh);
        }
    }
}

impl Default for WatcherService {
    fn default() -> Self {
        Self::new()
    }
}
