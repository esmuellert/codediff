//! A background thread that re-runs `get_files` on demand.

use std::sync::mpsc::{Receiver, Sender, channel};
use std::thread;

use channel::Emitter;
use file_types::File;

use super::{Request, get_files};

/// The list worker — re-runs the file list in the background when asked.
pub struct ListWorker {
    requests: Sender<Request>,
    queued: usize,
    queue_size: usize,
}

impl ListWorker {
    pub fn start(emitter: Emitter<Vec<File>>, queue_size: usize) -> Self {
        let (requests, incoming) = channel::<Request>();
        thread::Builder::new()
            .name("list".to_owned())
            .spawn(move || run(&incoming, &emitter))
            .expect("the list thread starts");
        Self {
            requests,
            queued: 0,
            queue_size,
        }
    }
}

impl channel::Worker for ListWorker {
    type Request = Request;
    type Response = Vec<File>;

    fn send(&mut self, request: Self::Request) {
        if self.queued >= self.queue_size {
            return;
        }
        if self.requests.send(request).is_ok() {
            self.queued += 1;
        }
    }

    fn is_busy(&self) -> bool {
        self.queued > 0
    }

    fn received(&mut self, _response: &Self::Response) {
        self.queued = 0;
    }
}

fn run(requests: &Receiver<Request>, answers: &Emitter<Vec<File>>) {
    while let Ok(mut request) = requests.recv() {
        // Drain to latest — only the newest request matters.
        while let Ok(newer) = requests.try_recv() {
            request = newer;
        }
        let files = get_files(&request).unwrap_or_default();
        if !answers.send(files) {
            break;
        }
    }
}
