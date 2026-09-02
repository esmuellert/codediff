//! The bridge between components and the syntax worker thread.

use std::cell::RefCell;
use std::rc::Rc;
use std::sync::Arc;

use channel::Worker;
use file_types::{DiffVersion, File};
use loom::{Observable, Observer, observable};
use syntax::{Store, Syntax, SyntaxRequest, Version, path_of};

pub struct SyntaxService {
    worker: Rc<RefCell<Syntax>>,
    store: RefCell<Store>,
    observer: RefCell<Option<Observer<Rc<Store>>>>,
    version: RefCell<Version>,
}

impl SyntaxService {
    pub fn new(worker: Rc<RefCell<Syntax>>) -> Self {
        Self {
            worker,
            store: RefCell::new(Store::new()),
            observer: RefCell::new(None),
            version: RefCell::new(Version(0)),
        }
    }

    /// Subscribes to syntax updates. The observable fires a snapshot of the
    /// store each time a chunk is installed.
    pub fn subscribe(&self) -> Observable<Rc<Store>> {
        let (observer, responses) = observable();
        *self.observer.borrow_mut() = Some(observer);
        responses
    }

    /// Bumps the version for cache invalidation.
    pub fn new_file(&self) {
        let mut v = self.version.borrow_mut();
        *v = Version(v.0 + 1);
    }

    /// Asks the worker to colour one version of one file up to `last`.
    pub fn request(&self, file: &File, version: DiffVersion, text: Arc<Vec<String>>, last: u32) {
        let syntax_version = *self.version.borrow();
        let (Some(key), Some(path)) = (file.name(version), path_of(file, version)) else {
            return;
        };
        let lines = text.len() as u32;
        if lines == 0 {
            return;
        }
        let last = last.min(lines - 1);
        let mut store = self.store.borrow_mut();
        let mut worker = self.worker.borrow_mut();
        if worker.busy(&key) {
            return;
        }
        store.ensure_cache(&key, syntax_version);
        let have = store.get_lines_coloured(&key);
        if have > last {
            return;
        }
        worker.send(SyntaxRequest {
            key,
            path,
            version: syntax_version,
            text,
            have,
            last,
        });
    }

    /// The loop calls this when the worker answered.
    pub fn deliver(&self, response: syntax::SyntaxResponse) {
        self.worker.borrow_mut().received(&response);
        let changed = self.store.borrow_mut().install(response);
        if changed && let Some(observer) = self.observer.borrow().as_ref() {
            observer.next(Rc::new(self.store.borrow().clone()));
        }
    }

    /// Reads the coloured spans for one line of one version.
    ///
    /// `line` is the line number as shown, counting from one.
    pub fn line_spans(
        store: &Store,
        file: &File,
        version: DiffVersion,
        line: u32,
    ) -> Vec<syntax::Span> {
        let key = match file.name(version) {
            Some(k) => k,
            None => return Vec::new(),
        };
        let Some(index) = line.checked_sub(1) else {
            return Vec::new();
        };
        match store.get_colours(&key) {
            Some(colours) => colours.line(index).to_vec(),
            None => Vec::new(),
        }
    }
}
