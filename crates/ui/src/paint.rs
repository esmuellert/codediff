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
//! colour.** It asks for one, draws whatever it has, and installs the answer
//! when it arrives.
//!
//! ---
//!
//! ```text
//!  drawing thread                        painter
//!  ──────────────                        ───────
//!  open a file
//!    Painter::paint(job) ──────────────► recv        ← asleep until sent to
//!  draw, with no colour                  colour it
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
//! **What crosses, and what cannot.** [`Highlighted`] holds a raw pointer from
//! the regex engine underneath `syntect` and is therefore not [`Send`] — so it
//! never leaves the painter. What crosses is the text going in and the spans
//! coming back, both of which are plain data. That is checked by the compiler
//! rather than by a comment.
//!
//! **Staleness.** A [`Version`] rides on every request and comes back on every
//! answer. Nothing today can invalidate a file mid-paint, but a file watcher
//! can, and an explorer can outrun one — so the answer to "is this still what
//! was asked for" is built in rather than retrofitted after the first bug.

use std::sync::OnceLock;
use std::sync::mpsc::{Receiver, Sender, TryRecvError, channel};
use std::thread;

use align::DiffVersion;
use file_types::File;
use syntax::{Clues, Engine, Highlighted, Palette, Span};

/// Which buffer, and which of its two sides, a request is about.
///
/// Opaque to the painter: it is handed back untouched so the drawing thread
/// can find what an answer belongs to.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Version(pub u64);

/// A file to colour.
///
/// Owns its lines rather than borrowing them: the painter outlives any
/// particular frame, and a borrow would tie it to whoever asked.
pub struct Job {
    pub version: Version,
    /// Which language, decided from the path on *this* side — a `.py` renamed
    /// to a `.rs` is Python on the left and Rust on the right, and showing
    /// either as the other would be a lie the reader can see.
    pub path: String,
    pub lines: Vec<String>,
}

/// Some of a file, coloured.
///
/// A file arrives in pieces, oldest first, because a reader should not watch
/// plain text for the sixteen seconds a three-hundred-thousand-line file takes.
/// `from` is the line the piece starts at, so the drawing thread appends
/// without needing to know how many pieces there will be.
pub struct Painted {
    pub version: Version,
    pub from: u32,
    pub spans: Vec<Vec<Span>>,
}

/// A file being coloured, as the drawing thread sees it.
///
/// Spans for the lines answered so far, and nothing else. Deliberately *not*
/// a [`Highlighted`]: that type cannot cross a thread, and the interface has
/// no business holding a highlighter when it is not the thing highlighting.
#[derive(Debug, Default)]
pub struct Colours {
    read: Vec<Vec<Span>>,
}

impl Colours {
    /// How the given line is coloured, or nothing if it has not arrived.
    ///
    /// Nothing is the ordinary answer for a line the painter has not reached,
    /// and means "draw it plainly" rather than "this line has no colour".
    pub fn line(&self, line: u32) -> &[Span] {
        self.read
            .get(line as usize)
            .map(Vec::as_slice)
            .unwrap_or_default()
    }

    /// Adds a piece the painter finished.
    ///
    /// Out-of-order and repeated pieces are ignored rather than trusted: the
    /// painter sends in order, but a stale answer must not be able to shorten
    /// or reorder what is already drawn.
    pub fn install(&mut self, painted: Painted) {
        if painted.from as usize != self.read.len() {
            return;
        }
        self.read.extend(painted.spans);
    }

    pub fn lines(&self) -> u32 {
        self.read.len() as u32
    }
}

/// The painter, and the two queues to it.
pub struct Painter {
    jobs: Sender<Job>,
    painted: Receiver<Painted>,
}

impl Painter {
    /// Starts the painter.
    ///
    /// One thread for the life of the program. It sleeps whenever there is
    /// nothing to colour, which is nearly always.
    pub fn start() -> Self {
        let (jobs, work) = channel::<Job>();
        let (finished, painted) = channel::<Painted>();
        thread::Builder::new()
            .name("painter".to_owned())
            .spawn(move || paint_until_closed(&work, &finished))
            .expect("the painter thread starts");
        Self { jobs, painted }
    }

    /// Asks for a file to be coloured.
    ///
    /// Returns without waiting. A painter that has stopped — which can only
    /// happen if it panicked — is not an error worth failing a review over:
    /// the file simply stays plain.
    pub fn paint(&self, job: Job) {
        let _ = self.jobs.send(job);
    }

    /// Waits for the next finished piece.
    ///
    /// Blocks. Only [`Session::settle`](crate::Session::settle) uses it, and
    /// only so a test can wait for the colours it is about to assert on; the
    /// interface itself must never wait for a colour.
    ///
    /// `None` means the painter has stopped, which can only happen if it
    /// panicked.
    pub fn next(&self) -> Option<Painted> {
        self.painted.recv().ok()
    }

    /// Everything the painter has finished since this was last called.
    ///
    /// Never blocks. Called once a frame, and costs a few nanoseconds when
    /// there is nothing.
    pub fn take(&self) -> Vec<Painted> {
        let mut out = Vec::new();
        loop {
            match self.painted.try_recv() {
                Ok(painted) => out.push(painted),
                Err(TryRecvError::Empty | TryRecvError::Disconnected) => return out,
            }
        }
    }
}

impl Default for Painter {
    fn default() -> Self {
        Self::start()
    }
}

/// Lines sent back at a time.
///
/// The only reason this is not "the whole file at once" is that a very long
/// file would otherwise stay plain until all of it was done — sixteen seconds
/// for three hundred thousand lines with the slower engine. Two thousand lines
/// is a hundred milliseconds of that engine's work and five of the other's, so
/// a reader sees colour arrive promptly and in order.
const CHUNK: usize = 2_000;

fn paint_until_closed(jobs: &Receiver<Job>, finished: &Sender<Painted>) {
    // Blocks. A painter with nothing to do costs nothing at all — no timer, no
    // spin, no wake-ups — which is the ordinary state of this thread.
    while let Ok(job) = jobs.recv() {
        paint_one(&job, finished);
    }
}

fn paint_one(job: &Job, finished: &Sender<Painted>) {
    let clues = Clues::new(&job.path, job.lines.first().map(String::as_str));
    let Some(grammar) = engine().find(clues, job.lines.len()) else {
        // Nothing claims this language. Answering once with no spans is how
        // the caller learns there is nothing coming.
        let _ = finished.send(Painted {
            version: job.version,
            from: 0,
            spans: vec![Vec::new(); job.lines.len()],
        });
        return;
    };

    let mut read = Highlighted::new(engine(), grammar, palette(), &job.lines);
    let mut sent = 0usize;
    while sent < job.lines.len() {
        let want = (sent + CHUNK).min(job.lines.len());
        read.reach(engine(), palette(), want as u32 - 1, &job.lines);

        // What the engine actually reached, which for a parser is the whole
        // file however little was asked for.
        let got = read.done() as usize;
        if got <= sent {
            // It cannot get any further, so neither can we. Fill the rest
            // plainly rather than leave the caller waiting for ever.
            let _ = finished.send(Painted {
                version: job.version,
                from: sent as u32,
                spans: vec![Vec::new(); job.lines.len() - sent],
            });
            return;
        }

        let spans = (sent..got).map(|n| read.line(n as u32).to_vec()).collect();
        if finished
            .send(Painted {
                version: job.version,
                from: sent as u32,
                spans,
            })
            .is_err()
        {
            // Nobody is listening any more, which means the review has ended.
            return;
        }
        sent = got;
    }
}

/// Every grammar, unpacked once.
fn engine() -> &'static Engine {
    static ENGINE: OnceLock<Engine> = OnceLock::new();
    ENGINE.get_or_init(Engine::new)
}

/// Both halves of the theme, compiled once.
///
/// Neither depends on the theme, because a span names a pen rather than a
/// colour — so changing theme invalidates nothing and re-reads nothing.
fn palette() -> &'static Palette {
    static PALETTE: OnceLock<Palette> = OnceLock::new();
    PALETTE.get_or_init(|| {
        Palette::new(
            &crate::theme::scopes::rules(),
            &crate::theme::captures::captures(),
        )
    })
}

/// The path a version of a file is known by, if it exists on that side.
///
/// A file added or deleted exists on one side only, and the side it does not
/// exist on has no text and therefore no language.
pub fn path_of(file: &File, version: DiffVersion) -> Option<String> {
    file.on(version).map(|path| path.as_str().to_owned())
}

/// The colouring of the versions a pane is drawing, for the frame.
///
/// Borrowed rather than owned so a renderer can be handed it without learning
/// what a diff is. `Off` is what the toggle produces and what a buffer with
/// nothing to colour reports; both draw plainly, and neither is a special case
/// anywhere below.
#[derive(Clone, Copy, Default)]
pub enum Spans<'a> {
    #[default]
    Off,
    /// One version, which is what a lone file has.
    One(&'a Colours),
    /// Both, which is what a diff has.
    Both {
        original: &'a Colours,
        modified: &'a Colours,
    },
}

impl<'a> Spans<'a> {
    /// How line `number` of one version is coloured.
    ///
    /// **Numbered from 1**, like [`Alignment::line`] and like the gutter, and
    /// unlike the spans underneath, which are indexed from 0. The two
    /// conventions meet here and nowhere else: written at each call site
    /// instead, it was wrong at one of them, and a whole file coloured one line
    /// out still looks coloured.
    ///
    /// [`Alignment::line`]: align::Alignment::line
    pub fn line(&self, version: DiffVersion, number: u32) -> &'a [Span] {
        let Some(index) = number.checked_sub(1) else {
            return &[];
        };
        match self {
            Spans::Off => &[],
            Spans::One(read) => read.line(index),
            Spans::Both { original, modified } => match version {
                DiffVersion::Original => original.line(index),
                DiffVersion::Modified => modified.line(index),
            },
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn lines(source: &str) -> Vec<String> {
        source.lines().map(str::to_owned).collect()
    }

    /// Asks for a file and waits for all of it, which is what a test wants and
    /// a frame never does.
    fn painted(path: &str, source: &str) -> Colours {
        let painter = Painter::start();
        let lines = lines(source);
        let wanted = lines.len();
        painter.paint(Job {
            version: Version(1),
            path: path.to_owned(),
            lines,
        });

        let mut colours = Colours::default();
        while (colours.lines() as usize) < wanted {
            for piece in painter.take() {
                colours.install(piece);
            }
        }
        colours
    }

    #[test]
    fn a_file_comes_back_coloured() {
        let colours = painted("src/main.rs", "fn main() {\n    let x = 1;\n}\n");
        assert!(!colours.line(0).is_empty(), "`fn` is a keyword");
        assert_eq!(colours.lines(), 3);
    }

    #[test]
    fn a_language_nobody_claims_still_answers_for_every_line() {
        // Otherwise the caller waits for ever for a file that will never be
        // coloured, which is a hang rather than a missing colour.
        let colours = painted("notes.qqzz", "one\ntwo\nthree\n");
        assert_eq!(colours.lines(), 3);
        assert!(colours.line(0).is_empty());
    }

    #[test]
    fn an_empty_file_answers_too() {
        let painter = Painter::start();
        painter.paint(Job {
            version: Version(1),
            path: "a.rs".to_owned(),
            lines: Vec::new(),
        });
        // Nothing to wait for, and nothing that hangs.
        let mut colours = Colours::default();
        for piece in painter.take() {
            colours.install(piece);
        }
        assert_eq!(colours.lines(), 0);
    }

    #[test]
    fn pieces_arrive_in_order_and_a_repeat_is_ignored() {
        // The painter sends in order, but a stale answer must not be able to
        // shorten what is already drawn.
        let mut colours = Colours::default();
        colours.install(Painted {
            version: Version(1),
            from: 0,
            spans: vec![Vec::new(); 3],
        });
        assert_eq!(colours.lines(), 3);

        colours.install(Painted {
            version: Version(1),
            from: 0,
            spans: vec![Vec::new(); 3],
        });
        assert_eq!(colours.lines(), 3, "a repeat changed nothing");

        colours.install(Painted {
            version: Version(1),
            from: 9,
            spans: vec![Vec::new(); 3],
        });
        assert_eq!(colours.lines(), 3, "and neither did a gap");
    }

    #[test]
    fn a_long_file_arrives_in_pieces() {
        let source: String = (0..CHUNK + 500)
            .map(|n| format!("fn f{n}() {{}}\n"))
            .collect();
        let painter = Painter::start();
        let lines = lines(&source);
        let wanted = lines.len();
        painter.paint(Job {
            version: Version(1),
            path: "a.rs".to_owned(),
            lines,
        });

        let mut colours = Colours::default();
        let mut pieces = 0;
        while (colours.lines() as usize) < wanted {
            for piece in painter.take() {
                pieces += 1;
                colours.install(piece);
            }
        }
        assert_eq!(colours.lines() as usize, wanted);
        // Rust is parsed, and a parser has no range API, so it answers with
        // the whole file however little was asked for. The piecing is there
        // for the engine that *can* stop, and for the file long enough to
        // need it.
        assert!(pieces >= 1);
    }

    #[test]
    fn the_painter_can_be_asked_twice() {
        let painter = Painter::start();
        for (n, path) in ["a.rs", "a.py"].into_iter().enumerate() {
            let lines = lines("fn f():\n    pass\n");
            let wanted = lines.len();
            painter.paint(Job {
                version: Version(n as u64),
                path: path.to_owned(),
                lines,
            });
            let mut colours = Colours::default();
            while (colours.lines() as usize) < wanted {
                for piece in painter.take() {
                    assert_eq!(piece.version, Version(n as u64));
                    colours.install(piece);
                }
            }
        }
    }

    #[test]
    fn taking_from_an_idle_painter_is_empty_rather_than_blocking() {
        let painter = Painter::start();
        assert!(painter.take().is_empty());
        assert!(painter.take().is_empty());
    }

    #[test]
    fn nothing_that_crosses_the_thread_holds_a_highlighter() {
        // The compiler already refuses otherwise — `Highlighted` is not `Send`
        // — but stating it here says *why* the model holds spans rather than a
        // highlighter, which is otherwise an odd-looking choice.
        fn sent<T: Send>() {}
        sent::<Job>();
        sent::<Painted>();
        sent::<Colours>();
    }
}
