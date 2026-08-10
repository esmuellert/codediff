//! Colouring a file on a separate thread.
//!
//! The worker sends results via an Emitter. In production it maps to the
//! event channel; in tests it maps to a local receiver.

mod message;
mod run;
mod store;

use std::collections::HashSet;
use std::sync::mpsc::{Sender, channel};
use std::thread;

use channel::{Emitter, Worker};

pub use message::{SyntaxRequest, SyntaxResponse, Version, path_of};
pub use store::{Colours, Spans, Store};

/// The worker handle. Holds only the request channel and outstanding set.
pub struct Syntax {
    requests: Sender<SyntaxRequest>,
    outstanding: HashSet<String>,
}

impl Syntax {
    /// Starts the worker thread with the given emitter for results.
    pub fn start(emitter: Emitter<SyntaxResponse>) -> Self {
        let (requests, incoming) = channel::<SyntaxRequest>();
        thread::Builder::new()
            .name("syntax".to_owned())
            .spawn(move || run::run(&incoming, &emitter))
            .expect("the syntax thread starts");
        Self {
            requests,
            outstanding: HashSet::new(),
        }
    }

    /// Whether a specific file is waiting on a response.
    pub fn busy(&self, key: &str) -> bool {
        self.outstanding.contains(key)
    }

    /// Clears a finished request, so the file can be asked about again.
    fn finish(&mut self, response: &SyntaxResponse) {
        if response.more {
            return;
        }
        self.outstanding.remove(&response.key);
    }
}

impl Worker for Syntax {
    type Request = SyntaxRequest;
    type Response = SyntaxResponse;

    fn send(&mut self, request: Self::Request) {
        if !self.outstanding.insert(request.key.clone()) {
            return;
        }
        let _ = self.requests.send(request);
    }

    fn is_busy(&self) -> bool {
        !self.outstanding.is_empty()
    }

    fn received(&mut self, response: &Self::Response) {
        self.finish(response);
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;
    use std::sync::mpsc;

    use channel::Emitter;

    use super::*;

    fn text(lines: usize) -> Arc<Vec<String>> {
        Arc::new((0..lines).map(|n| format!("let x{n} = {n};")).collect())
    }

    fn request(path: &str, text: &Arc<Vec<String>>, have: u32, last: u32) -> SyntaxRequest {
        SyntaxRequest {
            key: format!("worktree:{path}"),
            path: path.to_owned(),
            version: Version(1),
            text: Arc::clone(text),
            have,
            last,
        }
    }

    /// Creates a Syntax with a local receiver for testing.
    fn test_syntax() -> (Syntax, mpsc::Receiver<SyntaxResponse>) {
        let (tx, rx) = mpsc::channel();
        let emitter = Emitter::new(tx, std::convert::identity);
        (Syntax::start(emitter), rx)
    }

    /// Drains until nothing is outstanding.
    fn drain(syntax: &mut Syntax, rx: &mpsc::Receiver<SyntaxResponse>) -> Vec<SyntaxResponse> {
        let mut out = Vec::new();
        while syntax.is_busy() {
            match rx.recv() {
                Ok(response) => {
                    syntax.received(&response);
                    out.push(response);
                }
                Err(_) => break,
            }
        }
        out
    }

    #[test]
    fn a_request_is_answered() {
        let (mut syntax, rx) = test_syntax();
        let text = text(10);
        syntax.send(request("a.rs", &text, 0, 9));
        let answers = drain(&mut syntax, &rx);
        let lines: usize = answers.iter().map(|a| a.spans.len()).sum();
        assert_eq!(lines, 10, "every line asked for came back");
        assert!(!syntax.is_busy(), "and nothing is left outstanding");
    }

    #[test]
    fn a_second_request_for_one_file_is_dropped_rather_than_queued() {
        let (mut syntax, rx) = test_syntax();
        let text = text(20_000);
        syntax.send(request("a.pl", &text, 0, 19_999));
        for last in [100, 200, 300] {
            syntax.send(request("a.pl", &text, 0, last));
        }
        assert_eq!(syntax.outstanding.len(), 1, "one file, one request");
        drain(&mut syntax, &rx);
    }

    #[test]
    fn a_file_can_be_asked_about_again_once_its_answer_has_arrived() {
        let (mut syntax, rx) = test_syntax();
        let text = text(50);
        let key = "worktree:a.pl".to_owned();

        syntax.send(request("a.pl", &text, 0, 9));
        assert!(syntax.busy(&key));
        let first = drain(&mut syntax, &rx);
        assert!(!syntax.busy(&key), "and free again afterwards");

        let read: u32 = first.iter().map(|a| a.spans.len() as u32).sum();
        syntax.send(request("a.pl", &text, read, 49));
        let second = drain(&mut syntax, &rx);
        let total = read + second.iter().map(|a| a.spans.len() as u32).sum::<u32>();
        assert_eq!(total, 50, "the rest of the file arrived on the second ask");
    }

    #[test]
    fn two_files_do_not_hold_each_other_up() {
        let (mut syntax, rx) = test_syntax();
        let text = text(10);
        syntax.send(request("a.rs", &text, 0, 9));
        syntax.send(request("b.rs", &text, 0, 9));
        assert_eq!(syntax.outstanding.len(), 2, "one entry each");
        drain(&mut syntax, &rx);
    }

    #[test]
    fn a_file_no_language_claims_is_still_answered() {
        let (mut syntax, rx) = test_syntax();
        let text = text(4);
        syntax.send(request("mystery.qqqqq", &text, 0, 3));
        let answers = drain(&mut syntax, &rx);
        assert!(!answers.is_empty(), "answered, even if with nothing");
        assert!(!syntax.is_busy());
    }

    #[test]
    fn reading_further_continues_rather_than_starting_again() {
        let (mut syntax, rx) = test_syntax();
        let text = text(500);
        syntax.send(request("a.pl", &text, 0, 99));
        let first = drain(&mut syntax, &rx);
        let read: u32 = first.iter().map(|a| a.spans.len() as u32).sum();

        syntax.send(request("a.pl", &text, read, 499));
        let second = drain(&mut syntax, &rx);
        assert_eq!(
            second.first().map(|a| a.from),
            Some(read),
            "carried on from where it stopped"
        );
    }
}
