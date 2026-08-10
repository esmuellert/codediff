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

use std::sync::mpsc::{Receiver, Sender, TryRecvError, channel};
use std::thread;

use crate::file::runner::{DiffContent, Runner};
use channel::Worker;
use file_types::File;

/// What one request produced.
pub struct Response {
    /// Which request this answers — used to drop late responses.
    pub file: File,
    pub content: Result<DiffContent, String>,
}

/// The worker, the two queues to it, and whether one is outstanding.
pub struct FileWorker {
    requests: Sender<File>,
    answers: Receiver<Response>,
    /// At most one request in flight.
    outstanding: bool,
}

impl FileWorker {
    /// Starts the worker thread.
    pub fn start() -> Self {
        let (requests, incoming) = channel::<File>();
        let (finished, answers) = channel::<Response>();
        thread::Builder::new()
            .name("file".to_owned())
            .spawn(move || run(&incoming, &finished))
            .expect("the file thread starts");
        Self {
            requests,
            answers,
            outstanding: false,
        }
    }

    /// Mock worker for tests. A request past the end
    /// of the script fails.
    pub fn mock(script: Vec<Result<DiffContent, String>>) -> Self {
        let (requests, incoming) = channel::<File>();
        let (finished, answers) = channel::<Response>();
        thread::Builder::new()
            .name("file-canned".to_owned())
            .spawn(move || {
                let mut script = script.into_iter();
                while let Ok(file) = incoming.recv() {
                    let content = script
                        .next()
                        .unwrap_or_else(|| Err("nothing left in the script".to_owned()));
                    if finished.send(Response { file, content }).is_err() {
                        return;
                    }
                }
            })
            .expect("the file thread starts");
        Self {
            requests,
            answers,
            outstanding: false,
        }
    }

    /// Waits for the response. Blocks — only for tests.
    pub fn recv(&mut self) -> Option<Response> {
        let response = self.answers.recv().ok()?;
        self.outstanding = false;
        Some(response)
    }
}

impl Worker for FileWorker {
    type Request = File;
    type Response = Response;

    fn send(&mut self, file: Self::Request) {
        if self.outstanding {
            return;
        }
        self.outstanding = true;
        let _ = self.requests.send(file);
    }

    fn is_busy(&self) -> bool {
        self.outstanding
    }

    fn poll(&mut self) -> Option<Self::Response> {
        match self.answers.try_recv() {
            Ok(response) => {
                self.outstanding = false;
                Some(response)
            }
            Err(TryRecvError::Empty | TryRecvError::Disconnected) => None,
        }
    }
}

/// Answers requests until the sender is dropped.
fn run(requests: &Receiver<File>, answers: &Sender<Response>) {
    while let Ok(file) = requests.recv() {
        let content = compare(&file);
        if answers.send(Response { file, content }).is_err() {
            return;
        }
    }
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
