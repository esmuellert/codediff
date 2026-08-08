//! Colouring a file on a separate thread.
//!
//! The drawing thread never computes a colour — it asks, draws what it has,
//! and installs responses as they arrive.
//!
//! ```text
//!  drawing thread                        syntax worker
//!  ──────────────                        ─────────────
//!  Store  — every span                   Engine — both engines
//!
//!  need lines 0..50
//!    hit  → draw
//!    miss → send ─────────────────────►  recv
//!  draw with what there is               colour it
//!                                        send ─────┐
//!  loop {                                          │
//!    take() ◄──────────────────────────────────────┘
//!    install, draw
//!  }
//! ```
//!
//! No engine type crosses the boundary — enforced by `Send`.

mod message;
mod store;
mod worker;

use std::collections::HashSet;
use std::sync::mpsc::{Receiver, Sender, TryRecvError, channel};
use std::thread;

pub use message::{SyntaxRequest, SyntaxResponse, Version, path_of};
pub use store::{Colours, Spans, Store};

/// The worker, the two queues to it, and what is outstanding.
pub struct Syntax {
    requests: Sender<SyntaxRequest>,
    answers: Receiver<SyntaxResponse>,
    /// Files with a request in flight. At most one per file — newer scrolls
    /// replace the previous request rather than queueing behind it.
    outstanding: HashSet<String>,
}

impl Syntax {
    /// Starts the worker.
    ///
    /// One thread for the life of the program. It sleeps whenever there is
    /// nothing to colour, which is nearly always.
    pub fn start() -> Self {
        let (requests, incoming) = channel::<SyntaxRequest>();
        let (finished, answers) = channel::<SyntaxResponse>();
        thread::Builder::new()
            .name("syntax".to_owned())
            .spawn(move || worker::run(&incoming, &finished))
            .expect("the syntax thread starts");
        Self {
            requests,
            answers,
            outstanding: HashSet::new(),
        }
    }

    /// Asks for lines of a file to be coloured.
    ///
    /// Returns without waiting. A worker that has stopped — which can only
    /// happen if it panicked — is not an error worth failing a review over:
    /// the file simply stays plain.
    pub fn send(&mut self, request: SyntaxRequest) {
        if !self.outstanding.insert(request.key.clone()) {
            return;
        }
        let _ = self.requests.send(request);
    }

    /// Whether a file is waiting on a response.
    pub fn busy(&self, key: &str) -> bool {
        self.outstanding.contains(key)
    }

    /// Whether anything at all is outstanding.
    ///
    /// What decides whether the loop waits for a frame or for a key.
    pub fn working(&self) -> bool {
        !self.outstanding.is_empty()
    }

    /// Everything finished since this was last called.
    ///
    /// Never blocks. Called once a frame, and costs a few nanoseconds when
    /// there is nothing.
    pub fn take(&mut self) -> Vec<SyntaxResponse> {
        let mut out = Vec::new();
        loop {
            match self.answers.try_recv() {
                Ok(response) => {
                    self.finish(&response);
                    out.push(response);
                }
                Err(TryRecvError::Empty | TryRecvError::Disconnected) => return out,
            }
        }
    }

    /// Waits for the next finished piece.
    ///
    /// Blocks. Only [`Session::settle`](crate::Session::settle) uses it, and
    /// only so a test can wait for the colours it is about to assert on; the
    /// interface itself must never wait for a colour.
    ///
    /// `None` means the worker has stopped, which can only happen if it
    /// panicked.
    pub fn next(&mut self) -> Option<SyntaxResponse> {
        let response = self.answers.recv().ok()?;
        self.finish(&response);
        Some(response)
    }

    /// Clears a finished request, so the file can be asked about again.
    fn finish(&mut self, response: &SyntaxResponse) {
        if response.more {
            return;
        }
        self.outstanding.remove(&response.key);
    }
}

impl Default for Syntax {
    fn default() -> Self {
        Self::start()
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

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

    /// Drains until nothing is outstanding, as a frame loop eventually does.
    fn drain(syntax: &mut Syntax) -> Vec<SyntaxResponse> {
        let mut out = Vec::new();
        while syntax.working() {
            match syntax.next() {
                Some(response) => out.push(response),
                None => break,
            }
        }
        out
    }

    #[test]
    fn a_request_is_answered() {
        let mut syntax = Syntax::start();
        let text = text(10);
        syntax.send(request("a.rs", &text, 0, 9));
        let answers = drain(&mut syntax);
        let lines: usize = answers.iter().map(|a| a.spans.len()).sum();
        assert_eq!(lines, 10, "every line asked for came back");
        assert!(!syntax.working(), "and nothing is left outstanding");
    }

    #[test]
    fn a_second_request_for_one_file_is_dropped_rather_than_queued() {
        // Scrolling changes what is wanted sixty times a second. Sending each
        // change would build a queue of answers for screens already gone, and
        // keeping the newest would answer it from a starting point that had
        // moved. Neither: the asker re-asks when it next draws.
        let mut syntax = Syntax::start();
        let text = text(20_000);
        syntax.send(request("a.pl", &text, 0, 19_999));
        for last in [100, 200, 300] {
            syntax.send(request("a.pl", &text, 0, last));
        }
        assert_eq!(syntax.outstanding.len(), 1, "one file, one request");
        drain(&mut syntax);
    }

    #[test]
    fn a_file_can_be_asked_about_again_once_its_answer_has_arrived() {
        let mut syntax = Syntax::start();
        let text = text(50);
        let key = "worktree:a.pl".to_owned();

        syntax.send(request("a.pl", &text, 0, 9));
        assert!(syntax.busy(&key));
        let first = drain(&mut syntax);
        assert!(!syntax.busy(&key), "and free again afterwards");

        let read: u32 = first.iter().map(|a| a.spans.len() as u32).sum();
        syntax.send(request("a.pl", &text, read, 49));
        let second = drain(&mut syntax);
        let total = read + second.iter().map(|a| a.spans.len() as u32).sum::<u32>();
        assert_eq!(total, 50, "the rest of the file arrived on the second ask");
    }

    #[test]
    fn two_files_do_not_hold_each_other_up() {
        let mut syntax = Syntax::start();
        let text = text(10);
        syntax.send(request("a.rs", &text, 0, 9));
        syntax.send(request("b.rs", &text, 0, 9));
        assert_eq!(syntax.outstanding.len(), 2, "one entry each");
        drain(&mut syntax);
    }

    #[test]
    fn a_file_no_language_claims_is_still_answered() {
        // Otherwise the entry for it would never clear, and that file could
        // never be asked about again.
        let mut syntax = Syntax::start();
        let text = text(4);
        syntax.send(request("mystery.qqqqq", &text, 0, 3));
        let answers = drain(&mut syntax);
        assert!(!answers.is_empty(), "answered, even if with nothing");
        assert!(!syntax.working());
    }

    #[test]
    fn reading_further_continues_rather_than_starting_again() {
        // The lines already held must not come back a second time: the store
        // refuses a piece that does not continue where the last ended, so a
        // repeat would be dropped and the file would stop growing.
        let mut syntax = Syntax::start();
        let text = text(500);
        syntax.send(request("a.pl", &text, 0, 99));
        let first = drain(&mut syntax);
        let read: u32 = first.iter().map(|a| a.spans.len() as u32).sum();

        syntax.send(request("a.pl", &text, read, 499));
        let second = drain(&mut syntax);
        assert_eq!(
            second.first().map(|a| a.from),
            Some(read),
            "carried on from where it stopped"
        );
    }
}
