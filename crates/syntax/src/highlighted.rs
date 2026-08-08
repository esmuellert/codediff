//! A file being coloured from the top, as far as anyone has looked.
//!
//! Two fields: the engine's position and how far it got. Nothing is
//! invalidated — a file under review is a snapshot. Line 40's answer never
//! changes, so a prefix once read is read for good.
//!
//! Spans go straight to the caller; only the engine's position is held here.
//!
//! Both engines fit: the matcher resumes from where it stopped, the parser
//! reads the whole file on the first ask. `reach`/`done` is the interface.

use crate::engine::{Engine, Grammar, Palette, Reading};
use crate::limits;
use crate::style::Span;

/// One version of one file, coloured as far as it has been read.
pub struct Highlighted {
    /// How many lines from the top have been read.
    done: u32,
    /// Where the engine got to, or `None` once there is nothing more to do —
    /// either the file is finished, or it was never worth starting.
    reading: Option<Box<Reading>>,
}

impl std::fmt::Debug for Highlighted {
    /// Written out rather than derived because `Reading` is a grammar's
    /// context stack, which no failing test is easier to read for.
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Highlighted")
            .field("done", &self.done)
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
            done: 0,
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
            done: 0,
            reading: Some(Box::new(engine.start(grammar, palette))),
        }
    }

    /// How many lines have been read.
    pub fn done(&self) -> u32 {
        self.done
    }

    /// Whether there is anything left to read.
    pub fn finished(&self) -> bool {
        self.reading.is_none()
    }

    /// Reads until `line` has been coloured, appending to `into`.
    ///
    /// `into` receives one entry per line read by *this* call, so a caller
    /// draining it between calls gets each line exactly once. Reading back
    /// costs nothing because it does not happen: a line already read is a line
    /// the caller already has.
    ///
    /// May read further than asked. The parser has no range API, so it
    /// answers with the whole file however little was wanted;
    /// [`done`](Self::done) says what actually happened.
    pub fn reach(
        &mut self,
        engine: &Engine,
        palette: &Palette,
        line: u32,
        lines: &[String],
        into: &mut Vec<Vec<Span>>,
    ) {
        self.read_to(engine, palette, line as usize + 1, lines, into);
    }

    fn read_to(
        &mut self,
        engine: &Engine,
        palette: &Palette,
        target: usize,
        lines: &[String],
        into: &mut Vec<Vec<Span>>,
    ) {
        let target = target.min(lines.len());
        if self.done as usize >= target {
            return;
        }
        let Some(reading) = self.reading.as_mut() else {
            return;
        };
        // The range is what we *want*. One engine parses whole files and has
        // no way to do less, so it may come back having read everything —
        // which is why the check below is `>=` and not `==`.
        let before = into.len();
        let from = self.done as usize;
        engine.read(reading, palette, lines, from..target, into);
        self.done += (into.len() - before) as u32;
        if self.done as usize >= lines.len() {
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
        Palette::from_tables(
            &[Rule::new("keyword", word), Rule::new("storage", word)],
            &[Capture::new("keyword", word)],
        )
    }

    /// Everything a read needs, plus the buffer it writes into.
    struct Case {
        engine: Engine,
        palette: Palette,
        lines: Vec<String>,
        highlighted: Highlighted,
        spans: Vec<Vec<Span>>,
    }

    impl Case {
        fn reach(&mut self, line: u32) {
            self.highlighted.reach(
                &self.engine,
                &self.palette,
                line,
                &self.lines,
                &mut self.spans,
            );
        }
    }

    fn rust(lines: &[&str]) -> Case {
        let engine = Engine::new();
        let palette = palette();
        let owned: Vec<String> = lines.iter().map(|l| (*l).to_owned()).collect();
        // Whichever engine the seam picks. These tests are about *this* file
        // — how far it reads and what it hands back — and both engines go
        // through it, so naming one would be testing the seam instead.
        let grammar = engine
            .find(Clues::new("a.rs", None), lines.len())
            .expect("rust is a language");
        let highlighted = Highlighted::new(&engine, grammar, &palette, &owned);
        Case {
            engine,
            palette,
            lines: owned,
            highlighted,
            spans: Vec::new(),
        }
    }

    #[test]
    fn nothing_is_read_until_someone_looks() {
        let case = rust(&["fn a() {}", "fn b() {}"]);
        assert_eq!(case.highlighted.done(), 0);
        assert!(
            case.spans.is_empty(),
            "not read yet, so nothing handed back"
        );
    }

    #[test]
    fn reaching_a_line_reads_at_least_up_to_it() {
        let mut case = rust(&["fn a() {}", "fn b() {}", "fn c() {}"]);
        case.reach(1);
        assert!(case.highlighted.done() >= 2, "at least what was asked for");
        assert!(!case.spans[0].is_empty(), "`fn` is a keyword");
    }

    #[test]
    fn what_is_handed_back_matches_what_was_read() {
        // The count and the spans must agree, because the caller uses the
        // count to decide where the spans belong.
        let mut case = rust(&["fn a() {}", "fn b() {}", "fn c() {}"]);
        case.reach(2);
        assert_eq!(case.spans.len(), case.highlighted.done() as usize);
    }

    #[test]
    fn a_line_is_handed_back_once_and_only_once() {
        // Two calls covering overlapping ranges must not repeat a line, or
        // the caller would install it twice at two different places.
        let mut case = rust(&["fn a() {}", "fn b() {}", "fn c() {}"]);
        case.reach(0);
        let after_first = case.spans.len();
        case.reach(2);
        assert_eq!(
            case.spans.len(),
            case.highlighted.done() as usize,
            "the second call appended only what the first had not"
        );
        assert!(
            case.spans.len() >= after_first,
            "and never took anything back"
        );
    }

    #[test]
    fn reaching_a_line_already_read_does_nothing() {
        let mut case = rust(&["fn a() {}", "fn b() {}"]);
        case.reach(1);
        let spans = case.spans.clone();
        case.reach(0);
        assert_eq!(case.highlighted.done(), 2, "did not go backwards");
        assert_eq!(case.spans, spans, "and did not change its mind");
    }

    #[test]
    fn a_file_read_to_its_end_reports_finished() {
        let mut case = rust(&["fn a() {}"]);
        assert!(!case.highlighted.finished());
        case.reach(0);
        assert!(case.highlighted.finished(), "nothing left to carry forward");
    }

    #[test]
    fn reading_may_go_further_than_asked_but_never_less() {
        // The parser has no range API and answers with the whole file however
        // little was wanted, so a caller must look at `done` rather than
        // assume it got what it asked for.
        let mut case = rust(&["fn a() {}", "fn b() {}", "fn c() {}"]);
        case.reach(0);
        assert!(case.highlighted.done() >= 1, "at least the line asked for");
        assert!(
            case.highlighted.done() <= case.lines.len() as u32,
            "and never past the file"
        );
    }

    #[test]
    fn a_file_nobody_colours_reads_nothing_and_is_already_done() {
        let mut spans = Vec::new();
        let engine = Engine::new();
        let palette = palette();
        let mut h = Highlighted::none();
        assert!(h.finished());
        h.reach(&engine, &palette, 9_999, &[], &mut spans);
        assert_eq!(h.done(), 0);
        assert!(spans.is_empty(), "nothing to hand back");
    }
}
