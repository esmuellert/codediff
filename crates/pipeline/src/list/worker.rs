//! A background thread that re-runs `get_files` on demand.

use std::thread;

use channel::{Emitter, Slot};
use file_types::File;

use super::{Request, get_files};

/// The list worker — re-runs the file list in the background when asked.
pub struct ListWorker {
    requests: Slot<Request>,
}

impl ListWorker {
    pub fn start(emitter: Emitter<Vec<File>>) -> Self {
        let job = move |request: Request| {
            let files = get_files(&request).unwrap_or_default();
            emitter.send(files)
        };
        let (requests, worker_loop) = Slot::new(job);
        thread::Builder::new()
            .name("list".to_owned())
            .spawn(worker_loop)
            .expect("the list thread starts");
        Self { requests }
    }
}

impl channel::Worker for ListWorker {
    type Request = Request;
    type Response = Vec<File>;

    fn send(&mut self, request: Self::Request) {
        self.requests.put(request);
    }

    fn is_busy(&self) -> bool {
        self.requests.is_busy()
    }

    fn received(&mut self, _response: &Self::Response) {}
}
