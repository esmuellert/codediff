//! The bridge between a component and the file list worker thread.

use std::cell::RefCell;
use std::path::Path;
use std::rc::Rc;

use channel::Worker;
use file_types::File;
use loom::{Observable, Observer, observable};
use pipeline::list::{ListWorker, Request};

/// A way to get the list of changed files.
///
/// Created once in `lib.rs`, held by the provider. The provider subscribes
/// to the file list and to filesystem changes; the loop relays values from
/// the channel to the observers inside.
pub struct FileService {
    worker: Rc<RefCell<ListWorker>>,
    pathspec: Vec<String>,
    file_observer: RefCell<Option<Observer<Vec<File>>>>,
    fs_observer: RefCell<Option<Observer<watcher::Refresh>>>,
}

impl FileService {
    pub fn new(worker: Rc<RefCell<ListWorker>>, pathspec: Vec<String>) -> Self {
        Self {
            worker,
            pathspec,
            file_observer: RefCell::new(None),
            fs_observer: RefCell::new(None),
        }
    }

    /// Tells the worker to get the file list for `repo`.
    ///
    /// Returns an observable the caller subscribes to. Results keep arriving
    /// as the worktree changes.
    pub fn get(&self, repo: &Path) -> Observable<Vec<File>> {
        let (observer, responses) = observable();
        *self.file_observer.borrow_mut() = Some(observer);
        let request = Request::worktree(repo).with_pathspec(self.pathspec.clone());
        self.worker.borrow_mut().send(request);
        responses
    }

    /// Returns an observable that fires when the filesystem changes.
    ///
    /// The provider subscribes to this and re-calls `get` with its current
    /// repo path.
    pub fn on_fs_changed(&self) -> Observable<watcher::Refresh> {
        let (observer, responses) = observable();
        *self.fs_observer.borrow_mut() = Some(observer);
        responses
    }

    /// The loop calls this when the worker answered.
    pub fn deliver(&self, files: Vec<File>) {
        self.worker.borrow_mut().received(&files);
        if let Some(observer) = self.file_observer.borrow().as_ref() {
            observer.next(files);
        }
    }

    /// The loop calls this when the watcher fired.
    pub fn fs_changed(&self, what: watcher::Refresh) {
        if let Some(observer) = self.fs_observer.borrow().as_ref() {
            observer.next(what);
        }
    }
}
