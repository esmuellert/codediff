//! The bridge between a component and the file list worker thread.

use std::cell::RefCell;
use std::path::Path;
use std::rc::Rc;

use channel::Worker;
use loom::{Observable, Observer, observable};
use pipeline::files::{FilesWorker, Request, Response};

/// The worker-backed file list shared through context.
pub struct FilesService {
    worker: Rc<RefCell<FilesWorker>>,
    pathspec: Vec<String>,
    files_observer: RefCell<Option<Observer<Response>>>,
}

impl FilesService {
    pub fn new(worker: Rc<RefCell<FilesWorker>>, pathspec: Vec<String>) -> Self {
        Self {
            worker,
            pathspec,
            files_observer: RefCell::new(None),
        }
    }

    /// Requests the file list for `repo`.
    pub fn get(&self, repo: &Path) -> Observable<Response> {
        let (observer, responses) = observable();
        *self.files_observer.borrow_mut() = Some(observer);
        let request = Request::worktree(repo).with_pathspec(self.pathspec.clone());
        self.worker.borrow_mut().send(request);
        responses
    }

    /// Re-sends the request for the same repo. The existing subscriber
    /// receives the new result — no new observable needed.
    pub fn refresh(&self, repo: &Path) {
        let request = Request::worktree(repo).with_pathspec(self.pathspec.clone());
        self.worker.borrow_mut().send(request);
    }

    /// The loop calls this when the worker answered.
    pub fn deliver(&self, response: Response) {
        self.worker.borrow_mut().received(&response);
        if let Some(observer) = self.files_observer.borrow().as_ref() {
            observer.next(response);
        }
    }
}
