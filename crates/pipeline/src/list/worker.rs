//! A background thread that re-runs `get_files` on demand.

use std::sync::mpsc::{Receiver, Sender, TryRecvError, channel};
use std::thread;

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

    /// Sends a re-list request if no request is already in flight.
    pub fn send_request(&mut self, request: Request) {
        if self.outstanding {
            return;
        }
        if self.requests.send(request).is_ok() {
            self.outstanding = true;
        }
    }

    /// Returns a new file list if one is ready. Never blocks.
    pub fn poll(&mut self) -> Option<Vec<File>> {
        match self.answers.try_recv() {
            Ok(files) => {
                self.outstanding = false;
                Some(files)
            }
            Err(TryRecvError::Empty | TryRecvError::Disconnected) => None,
        }
    }

    pub fn is_busy(&self) -> bool {
        self.outstanding
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
