//! Colouring a file, on a thread that is not the one drawing.
//!
//! ---
//!
//! **Why a thread at all.** Colouring cannot be divided into pieces small
//! enough to hide between keystrokes. Preparing a language costs
//! `tree_sitter` a single indivisible 16–250 ms, and a parse has no range API,
//! so "do a little now and the rest later" is not on offer. The previous
//! design tried anyway — a frame did what it could and an idle moment did a
//! little more — and the result was that pressing a key during the first
//! Haskell file of a session waited 186 ms. Measured, not feared.
//!
//! So the work moves off the drawing thread entirely, and the rule becomes
//! simple enough to state in one line: **the interface never computes a
//! colour.** It asks for one, draws whatever it has, and installs the response
//! when it arrives.
//!
//! ---
//!
//! ```text
//!  drawing thread                        syntax worker
//!  ──────────────                        ─────────────
//!  Store  — every colour                 Engine — both engines
//!                                        memos  — places in unfinished files
//!
//!  need lines 0..50
//!    hit  → draw. nothing is sent.
//!    miss → send ─────────────────────►  recv     ← asleep until sent to
//!  draw, with what there is              colour it
//!                                        send ─────┐
//!  loop {                                          │
//!    take() ◄──────────────────────────────────────┘
//!    install, draw
//!    wait for a key                     (asleep again)
//!  }
//! ```
//!
//! One thread, one queue each way. Not `async`: there is one job and it is
//! processor-bound, so a runtime would add a scheduler with nothing to
//! schedule.
//!
//! **What crosses.** Text going in and spans coming back, both plain data in
//! our own vocabulary. No engine type appears in [`message`] — which is not
//! only tidiness, since `Highlighted` holds a pointer from the C regex library
//! underneath the matcher and would fail to be [`Send`]. The seam is checked
//! by the compiler rather than by a comment.
//!
//! **Staleness.** A [`Version`] rides on every request and comes back on every
//! response. Nothing today can invalidate a file mid-read, but a file watcher
//! can, and an explorer can outrun one — so the answer to "is this still what
//! was asked for" is built in rather than retrofitted after the first bug.

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
    /// One request in flight per file, and no queue behind it.
    ///
    /// Scrolling changes what is wanted on every frame, and sending each
    /// change would put sixty requests a second behind one that takes a
    /// quarter of a second to answer — the queue would grow for as long as
    /// the reader kept scrolling, and every response would be for a screen
    /// already gone.
    ///
    /// **Holding the newest one back would be worse, not better.** A request
    /// says how much of the file the asker already has, and that number moves
    /// every time a response lands — so a request waiting its turn is answered
    /// from a starting point that has since gone stale, and its lines are
    /// refused on arrival. Nothing is held. The asker re-asks on the next
    /// frame with a number that is current, and asking is a lookup.
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
