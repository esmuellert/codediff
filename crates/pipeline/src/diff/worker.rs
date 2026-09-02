//! The file worker thread — diffs one file at a time off the drawing thread.
//!
//! ```text
//!  drawing thread                        file worker thread
//!  ──────────────                        ──────────────────
//!  send_diff_request(file) ────────────────────────►  recv(file)
//!  draw (still showing previous file)    read, diff, align
//!  loop {                                send(result) ───┐
//!    poll() ◄────────────────────────────────────────────┘
//!    install, draw
//!  }
//! ```

use std::thread;

use crate::diff::runner::{DiffContent, Runner};
use channel::{Emitter, Slot, Worker};
use file_types::File;

/// What one request produced.
pub struct Response {
    /// Which request this answers — used to drop late responses.
    pub file: File,
    pub content: Result<DiffContent, String>,
}

/// The worker and the slot to it.
pub struct DiffWorker {
    requests: Slot<File>,
}

impl DiffWorker {
    /// Starts the worker thread.
    pub fn start(emitter: Emitter<Response>) -> Self {
        let job = move |file: File| {
            let content = compare(&file);
            tracing::info!(path = %file.path(), "file ready");
            emitter.send(Response { file, content })
        };
        let (requests, worker_loop) = Slot::new(job);
        thread::Builder::new()
            .name("file".to_owned())
            .spawn(worker_loop)
            .expect("the file thread starts");
        Self { requests }
    }

    /// Mock worker for tests.
    pub fn mock(script: Vec<Result<DiffContent, String>>, emitter: Emitter<Response>) -> Self {
        let mut script = script.into_iter();
        let job = move |file: File| {
            let content = script
                .next()
                .unwrap_or_else(|| Err("nothing left in the script".to_owned()));
            emitter.send(Response { file, content })
        };
        let (requests, worker_loop) = Slot::new(job);
        thread::Builder::new()
            .name("file-canned".to_owned())
            .spawn(worker_loop)
            .expect("the file thread starts");
        Self { requests }
    }
}

impl Worker for DiffWorker {
    type Request = File;
    type Response = Response;

    fn send(&mut self, file: Self::Request) {
        self.requests.put(file);
    }

    fn is_busy(&self) -> bool {
        self.requests.is_busy()
    }

    fn received(&mut self, _response: &Self::Response) {}
}

/// Runs the four stages. Nothing is cached between calls. See D51.
fn compare(file: &File) -> Result<DiffContent, String> {
    let path = file.path().as_str().to_owned();
    let runner = Runner::new(file).map_err(|why| format!("{path}: {why:#}"))?;
    if runner.is_binary() {
        return Err(format!("{path} is binary — there are no lines to review"));
    }
    runner
        .compute_diff()
        .map_err(|why| format!("{path}: {why:#}"))
}
