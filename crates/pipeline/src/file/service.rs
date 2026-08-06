//! The four stages, on a thread of their own.
//!
//! Not a stage — the thing that runs them. It is here rather than in `ui`
//! because what crosses the queue is this pipeline's vocabulary, and a worker
//! belongs beside the work it does.
//!
//! **Why a thread.** Comparing is not divisible into pieces small enough to
//! hide between keystrokes: there is nothing to show until the pairing exists,
//! so a half-finished diff has nothing to draw. Measured on a 50,000-line file
//! with one line in ten changed, the four stages take 1057 ms — of which the
//! engine is 718 ms — and the engine's own ceiling
//! ([`max_computation_time_ms`], 5 s) bounds the worst case far above that. On
//! the drawing thread every one of those milliseconds is a terminal that
//! answers no keys.
//!
//! So the rule is the same one colouring follows: **the interface never
//! compares.** It asks, draws whatever it already has, and installs the answer
//! when it arrives.
//!
//! ```text
//!  drawing thread                        file worker
//!  ──────────────                        ───────────
//!  reader presses enter
//!    want() ─────────────────────────►   recv     ← asleep until sent to
//!  draw, still showing the old file      four stages
//!  loop {                                send ─────┐
//!    take() ◄──────────────────────────────────────┘
//!    install, draw
//!    wait for a key                     (asleep again)
//!  }
//! ```
//!
//! [`max_computation_time_ms`]: vscode_diff::Options

use std::sync::mpsc::{Receiver, Sender, TryRecvError, channel};
use std::thread;

use crate::file::runner::{DiffContent, Runner};
use file_types::ChangedFile;

/// What one request produced.
///
/// A failure is a sentence, not an [`anyhow::Error`]: it is shown to a reader,
/// who has no use for a chain of contexts.
pub struct Answer {
    /// Which request this answers.
    ///
    /// Carried back so a late answer for a file the reader has since moved off
    /// can be told apart and dropped. Nothing today can invalidate a file
    /// mid-read, but a watcher can, and an explorer can outrun one.
    pub file: ChangedFile,
    pub content: Result<DiffContent, String>,
}

/// The worker, the two queues to it, and whether one is outstanding.
pub struct Files {
    wanted: Sender<ChangedFile>,
    answers: Receiver<Answer>,
    /// One request in flight, and no queue behind it.
    ///
    /// A reader holding `j` down moves through a list faster than any of it
    /// can be compared. Queueing would run git once per row and answer every
    /// one of them into a screen that had already moved on; the newest request
    /// is the only one anybody wants. So nothing is queued — the asker re-asks
    /// once the answer it is waiting for lands, with whatever row is current
    /// by then.
    outstanding: bool,
}

impl Files {
    /// Starts the worker.
    ///
    /// One thread for the life of the program. It sleeps whenever there is
    /// nothing to compare, which is nearly always.
    pub fn start() -> Self {
        let (wanted, incoming) = channel::<ChangedFile>();
        let (finished, answers) = channel::<Answer>();
        thread::Builder::new()
            .name("file".to_owned())
            .spawn(move || run(&incoming, &finished))
            .expect("the file thread starts");
        Self {
            wanted,
            answers,
            outstanding: false,
        }
    }

    /// A worker that answers from a script, in order, without touching git.
    ///
    /// **For tests, and for nothing else.** A test about what a pane shows
    /// needs a file read into it, not a repository on disk. It is a real thread
    /// answering a real queue, so what such a test exercises is the path the
    /// interface uses rather than a shortcut past it; only the four stages are
    /// replaced. A request past the end of the script is refused, which is how
    /// a test that opens more often than it meant to is told.
    pub fn canned(script: Vec<Result<DiffContent, String>>) -> Self {
        let (wanted, incoming) = channel::<ChangedFile>();
        let (finished, answers) = channel::<Answer>();
        thread::Builder::new()
            .name("file-canned".to_owned())
            .spawn(move || {
                let mut script = script.into_iter();
                while let Ok(file) = incoming.recv() {
                    let content = script
                        .next()
                        .unwrap_or_else(|| Err("nothing left in the script".to_owned()));
                    if finished.send(Answer { file, content }).is_err() {
                        return;
                    }
                }
            })
            .expect("the file thread starts");
        Self {
            wanted,
            answers,
            outstanding: false,
        }
    }

    /// Asks for a file to be compared.
    ///
    /// Returns without waiting — an unbounded channel never blocks the sender.
    /// Does nothing while an answer is outstanding, which is what keeps the
    /// queue at one. A worker that has stopped, which can only happen if it
    /// panicked, leaves the request unanswered rather than failing the review.
    pub fn want(&mut self, file: &ChangedFile) {
        if self.outstanding {
            return;
        }
        self.outstanding = true;
        let _ = self.wanted.send(file.clone());
    }

    /// Whether an answer is outstanding.
    ///
    /// What decides whether the loop waits for a frame or for a key.
    pub fn working(&self) -> bool {
        self.outstanding
    }

    /// The answer, if one has arrived.
    ///
    /// Never blocks. Called once a frame, and costs a few nanoseconds when
    /// there is nothing.
    pub fn take(&mut self) -> Option<Answer> {
        match self.answers.try_recv() {
            Ok(answer) => {
                self.outstanding = false;
                Some(answer)
            }
            Err(TryRecvError::Empty | TryRecvError::Disconnected) => None,
        }
    }

    /// Waits for the answer.
    ///
    /// **Blocks.** Only a caller with nothing else to do may use it: `debug
    /// diff-file` prints one file and exits, and a test waits for the
    /// content it is about to assert on. The interface must never wait for
    /// a file to be read — that is the whole reason this is on a thread.
    ///
    /// `None` means the worker has stopped, which can only happen if it
    /// panicked.
    pub fn wait(&mut self) -> Option<Answer> {
        let answer = self.answers.recv().ok()?;
        self.outstanding = false;
        Some(answer)
    }
}

/// Answers requests until the asker goes away.
fn run(requests: &Receiver<ChangedFile>, answers: &Sender<Answer>) {
    // Blocks. A worker with nothing to do costs nothing at all — no timer, no
    // spin, no wake-ups — which is its ordinary state.
    while let Ok(file) = requests.recv() {
        let content = compare(&file);
        if answers.send(Answer { file, content }).is_err() {
            return;
        }
    }
}

/// The four stages, with any failure turned into a sentence.
///
/// A sentence rather than an [`anyhow::Error`] because the interface shows it
/// to a reader on the status line and cannot format a chain of contexts.
/// `debug diff-file` drives [`Runner`] itself and keeps the chain.
///
/// **Nothing is kept between calls.** Three of the four revisions a row can
/// name — the working tree, the index, and a conflict stage — are mutable:
/// their bytes change while the review is open, and nothing in their name
/// changes with them. A cache keyed by those names cannot tell a re-read from
/// a stale one. Reading two versions and pairing them takes milliseconds,
/// which is the whole cost of getting this right. See D51.
fn compare(wanted: &ChangedFile) -> Result<DiffContent, String> {
    let path = wanted.path().as_str().to_owned();
    let runner = Runner::new(wanted).map_err(|why| format!("{path}: {why:#}"))?;
    if runner.is_binary() {
        return Err(format!("{path} is binary — there are no lines to review"));
    }
    runner.run().map_err(|why| format!("{path}: {why:#}"))
}
