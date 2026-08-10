//! A background thread that re-runs `get_files` on demand.

use std::sync::mpsc::{Receiver, Sender, TryRecvError, channel};
use std::thread;

use channel::Worker;
use file_types::File;

use super::{Request, get_files};

/// The list worker — re-runs the file list in the background when asked.
pub struct ListWorker {
    requests: Sender<Request>,
    answers: Receiver<Vec<File>>,
    outstanding: bool,
}

impl ListWorker {
    pub fn start() -> Self {
        let (requests, incoming) = channel::<Request>();
        let (finished, answers) = channel::<Vec<File>>();
        thread::Builder::new()
            .name("list".to_owned())
            .spawn(move || run(&incoming, &finished))
            .expect("the list thread starts");
        Self {
            requests,
            answers,
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

    fn poll(&mut self) -> Option<Self::Response> {
        match self.answers.try_recv() {
            Ok(files) => {
                self.outstanding = false;
                Some(files)
            }
            Err(TryRecvError::Empty | TryRecvError::Disconnected) => None,
        }
    }
}

fn run(requests: &Receiver<Request>, answers: &Sender<Vec<File>>) {
    while let Ok(request) = requests.recv() {
        let files = get_files(&request).unwrap_or_default();
        if answers.send(files).is_err() {
            break;
        }
    }
}
