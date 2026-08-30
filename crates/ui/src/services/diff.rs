//! The bridge between a component and the diff worker thread.

use std::cell::RefCell;
use std::rc::Rc;

use channel::Worker;
use file_types::File;
use loom::{Observable, Observer, observable};
use pipeline::diff::{DiffWorker, Response};

pub struct DiffService {
    worker: Rc<RefCell<DiffWorker>>,
    observer: RefCell<Option<Observer<Response>>>,
}

impl DiffService {
    pub fn new(worker: Rc<RefCell<DiffWorker>>) -> Self {
        Self {
            worker,
            observer: RefCell::new(None),
        }
    }

    pub fn get(&self, file: &File) -> Observable<Response> {
        let (observer, responses) = observable();
        *self.observer.borrow_mut() = Some(observer);
        self.worker.borrow_mut().send(file.clone());
        responses
    }

    pub fn deliver(&self, response: Response) {
        self.worker.borrow_mut().received(&response);
        if let Some(observer) = self.observer.borrow().as_ref() {
            observer.next(response);
        }
    }
}
