//! A file being coloured from the top, as far as anyone has looked.
//!
//! This is the whole of what VS Code needs a state store, an invalidation
//! queue and a convergence check for — and it is two fields, because we never
//! invalidate. A file under review is a snapshot: a git blob, or a worktree
//! file as it was when we read it. Nothing is typed into it, so the answer for
//! line 40 never changes, and a prefix once read is read for good.
//!
//! What remains is that the answer for line 500 depends on lines 1 to 499, so
//! reading can only ever go **forwards**. Scrolling back is free; jumping
//! ahead costs the gap, once.
//!
//! **Both engines fit here, and only one of them is lazy.** The matcher stops
//! where it is asked and resumes later; the parser has no way to read part of
//! a file and returns the whole thing on the first ask. Nothing in this file
//! branches on which: [`reach`](Self::reach) says how far it would like to get
//! and [`done`](Self::done) says how far it actually got.
//!
//! **Nothing here is scheduled against frames.** It once was — a frame read
//! what it could and an idle moment read a little more — and that could not
//! survive an engine whose smallest unit of work is an indivisible quarter of
//! a second. Colouring now happens on a thread of its own, so this may take as
//! long as it takes. See D41.

use crate::engine::{Engine, Grammar, Palette, Reading};
use crate::limits;
use crate::style::Span;

/// One version of one file, coloured as far as it has been read.
pub struct Highlighted {
    /// Spans for lines `0..read.len()`, in order.
    read: Vec<Vec<Span>>,
    /// Where the engine got to, or `None` once there is nothing more to do —
    /// either the file is finished, or it was never worth starting.
    reading: Option<Box<Reading>>,
}

impl std::fmt::Debug for Highlighted {
    /// How far it has got, not every span it found.
    ///
    /// Written out rather than derived because the derived form is tens of
    /// thousands of byte ranges, which no failing test is easier to read for.
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Highlighted")
            .field("read", &self.read.len())
            .field("finished", &self.finished())
            .finish()
    }
}

impl Highlighted {
    /// A file nobody will colour.
    ///
    /// What an unrecognised language, a binary file and a database dump all
    /// get. Not an error: every caller must already cope with having no spans
    /// for a line, because the first frame is drawn before the whole file has
    /// been read.
    pub fn none() -> Self {
        Self {
            read: Vec::new(),
            reading: None,
        }
    }

    /// Begins colouring a file, if it is worth colouring.
    pub fn new(engine: &Engine, grammar: Grammar, palette: &Palette, lines: &[String]) -> Self {
        let bytes = lines.iter().map(|line| line.len() + 1).sum();
        if !limits::worth_highlighting(bytes, lines.len()) {
            return Self::none();
        }
        Self {
            read: Vec::with_capacity(lines.len()),
            reading: Some(Box::new(engine.start(grammar, palette))),
        }
    }

    /// How the given line is coloured, or nothing if it has not been read.
    ///
    /// Nothing is the ordinary answer for a line below the point reached so
    /// far, and means "draw it plainly" rather than "this line has no colour".
    pub fn line(&self, line: u32) -> &[Span] {
        self.read
            .get(line as usize)
            .map(Vec::as_slice)
            .unwrap_or_default()
    }

    /// How many lines have been read.
    pub fn done(&self) -> u32 {
        self.read.len() as u32
    }

    /// Whether there is anything left to read.
    pub fn finished(&self) -> bool {
        self.reading.is_none()
    }

    /// Reads until `line` has been coloured.
    ///
    /// Cheap when the answer is already known, which after the first ask it
    /// usually is: results are kept, so reading further extends them and
    /// reading back costs nothing.
    ///
    /// May read **further** than asked. The parser has no range API, so it
    /// answers with the whole file however little was wanted;
    /// [`done`](Self::done) says what actually happened.
    pub fn reach(&mut self, engine: &Engine, palette: &Palette, line: u32, lines: &[String]) {
        self.read_to(engine, palette, line as usize + 1, lines);
    }

    fn read_to(&mut self, engine: &Engine, palette: &Palette, target: usize, lines: &[String]) {
        let target = target.min(lines.len());
        if self.read.len() >= target {
            return;
        }
        let Some(reading) = self.reading.as_mut() else {
            return;
        };
        // The range is what we *want*. One engine parses whole files and has
        // no way to do less, so it may come back having read everything —
        // which is why the check below is `>=` and not `==`.
        let from = self.read.len();
        engine.read(reading, palette, lines, from..target, &mut self.read);
        if self.read.len() >= lines.len() {
            // Nothing left to carry forward. Dropping it returns the grammar's
            // context stack, which for a deeply nested file is not nothing.
            self.reading = None;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::detect::Clues;
    use crate::style::{Capture, Pen, Rule, Style};

    fn palette() -> Palette {
        // `storage` as well as `keyword`, because Rust's `fn` is
        // `storage.type.function` — the kind of thing a scope path knows and a
        // fixed list of token names does not.
        let word = Style::pen(Pen(0));
        Palette::new(
            &[Rule::new("keyword", word), Rule::new("storage", word)],
            &[Capture::new("keyword", word)],
        )
    }

    fn rust(lines: &[&str]) -> (Engine, Palette, Vec<String>, Highlighted) {
        let engine = Engine::new();
        let palette = palette();
        let owned: Vec<String> = lines.iter().map(|l| (*l).to_owned()).collect();
        // Whichever engine the seam picks. These tests are about *this* file
        // — what it caches and how far it reads — and both engines go through
        // it, so naming one would be testing the seam instead.
        let grammar = engine
            .find(Clues::new("a.rs", None), lines.len())
            .expect("rust is a language");
        let highlighted = Highlighted::new(&engine, grammar, &palette, &owned);
        (engine, palette, owned, highlighted)
    }

    #[test]
    fn nothing_is_read_until_someone_looks() {
        let (_, _, _, h) = rust(&["fn a() {}", "fn b() {}"]);
        assert_eq!(h.done(), 0);
        assert!(h.line(0).is_empty(), "not read yet, so nothing to say");
    }

    #[test]
    fn reaching_a_line_reads_at_least_up_to_it() {
        let (engine, palette, lines, mut h) = rust(&["fn a() {}", "fn b() {}", "fn c() {}"]);
        h.reach(&engine, &palette, 1, &lines);
        assert!(h.done() >= 2, "at least what was asked for");
        assert!(!h.line(0).is_empty(), "`fn` is a keyword");
    }

    #[test]
    fn reaching_a_line_already_read_does_nothing() {
        let (engine, palette, lines, mut h) = rust(&["fn a() {}", "fn b() {}"]);
        h.reach(&engine, &palette, 1, &lines);
        let spans = h.line(0).to_vec();
        h.reach(&engine, &palette, 0, &lines);
        assert_eq!(h.done(), 2, "did not go backwards");
        assert_eq!(h.line(0), spans, "and did not change its mind");
    }

    #[test]
    fn a_file_read_to_its_end_reports_finished() {
        let (engine, palette, lines, mut h) = rust(&["fn a() {}"]);
        assert!(!h.finished());
        h.reach(&engine, &palette, 0, &lines);
        assert!(h.finished(), "nothing left to carry forward");
    }

    #[test]
    fn reading_may_go_further_than_asked_but_never_less() {
        // The parser has no range API and answers with the whole file however
        // little was wanted, so a caller must look at `done` rather than
        // assume it got what it asked for.
        let (engine, palette, lines, mut h) = rust(&["fn a() {}", "fn b() {}", "fn c() {}"]);
        h.reach(&engine, &palette, 0, &lines);
        assert!(h.done() >= 1, "at least the line asked for");
        assert!(h.done() <= lines.len() as u32, "and never past the file");
    }

    #[test]
    fn a_file_nobody_colours_answers_for_every_line() {
        let h = Highlighted::none();
        assert!(h.finished());
        assert!(h.line(0).is_empty());
        assert!(h.line(9_999).is_empty());
    }
}
