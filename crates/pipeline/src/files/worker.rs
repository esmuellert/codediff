//! A background thread that re-runs `get_files` on demand.

use std::thread;

use channel::{Emitter, Slot};
use file_types::File;

use super::{Request, Response, get_files};

/// Re-runs the changed-files request in the background.
pub struct FilesWorker {
    requests: Slot<Request>,
}

impl FilesWorker {
    pub fn start(emitter: Emitter<Response>) -> Self {
        Self::spawn(
            move |request| {
                let files = get_files(&request).unwrap_or_default();
                emitter.send(Response {
                    repo: request.repo,
                    files,
                })
            },
            "files",
        )
    }

    pub fn mock(script: Vec<Vec<File>>, emitter: Emitter<Response>) -> Self {
        let mut script = script.into_iter();
        Self::spawn(
            move |request| {
                emitter.send(Response {
                    repo: request.repo,
                    files: script.next().unwrap_or_default(),
                })
            },
            "files-canned",
        )
    }

    fn spawn(job: impl FnMut(Request) -> bool + Send + 'static, thread_name: &str) -> Self {
        let (requests, worker_loop) = Slot::new(job);
        thread::Builder::new()
            .name(thread_name.to_owned())
            .spawn(worker_loop)
            .expect("the files thread starts");
        Self { requests }
    }
}

impl channel::Worker for FilesWorker {
    type Request = Request;
    type Response = Response;

    fn send(&mut self, request: Self::Request) {
        self.requests.put(request);
    }

    fn is_busy(&self) -> bool {
        self.requests.is_busy()
    }

    fn received(&mut self, _response: &Self::Response) {}
}
