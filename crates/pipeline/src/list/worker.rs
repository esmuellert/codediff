//! A background thread that re-runs `get_files` on demand.

use std::sync::mpsc::{Receiver, Sender, channel};
use std::thread;

use channel::{Emitter, Worker};
use file_types::File;

use super::{Request, get_files};

/// The list worker — re-runs the file list in the background when asked.
pub struct ListWorker {
    requests: Sender<Request>,
    outstanding: bool,
}

impl ListWorker {
    pub fn start(emitter: Emitter<Vec<File>>) -> Self {
        let (requests, incoming) = channel::<Request>();
        thread::Builder::new()
            .name("list".to_owned())
            .spawn(move || run(&incoming, &emitter))
            .expect("the list thread starts");
        Self {
            requests,
            outstanding: false,
        }
    }
}

impl Worker for ListWorker {
    type Request = Request;
    type Response = Vec<File>;

    fn send(&mut self, request: Self::Request) {
        if self.outstanding {
            return;
        }
        if self.requests.send(request).is_ok() {
            self.outstanding = true;
        }
    }

    fn is_busy(&self) -> bool {
        self.outstanding
    }

    fn received(&mut self, _response: &Self::Response) {
        self.outstanding = false;
    }
}

fn run(requests: &Receiver<Request>, answers: &Emitter<Vec<File>>) {
    while let Ok(request) = requests.recv() {
        let files = get_files(&request).unwrap_or_default();
        if !answers.send(files) {
            break;
        }
    }
}
