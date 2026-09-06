//! Incremental syntax progress for one file snapshot.
//!
//! Parser state and spans are retained between requests.

use crate::engine::{Engine, EngineState, Grammar, Palette};
use crate::limits;
use crate::style::Span;

/// One version of one file, coloured as far as it has been read.
pub struct Highlighted {
    /// How many lines from the top have been read.
    lines_coloured: u32,
    /// Parser state, or `None` when reading is complete or disabled.
    engine_state: Option<Box<EngineState>>,
}

impl std::fmt::Debug for Highlighted {
    /// Avoid exposing the engine's internal parser state.
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Highlighted")
            .field("lines_coloured", &self.lines_coloured)
            .field("finished", &self.finished())
            .finish()
    }
}

impl Highlighted {
    /// Creates an empty progress value for a file with no syntax engine.
    pub fn none() -> Self {
        Self {
            lines_coloured: 0,
            engine_state: None,
        }
    }

    /// Begins colouring a file when it is within the highlighting limits.
    pub fn new(engine: &Engine, grammar: Grammar, palette: &Palette, lines: &[String]) -> Self {
        let bytes = lines.iter().map(|line| line.len() + 1).sum();
        if !limits::worth_highlighting(bytes, lines.len()) {
            return Self::none();
        }
        Self {
            lines_coloured: 0,
            engine_state: Some(Box::new(engine.start(grammar, palette))),
        }
    }

    /// How many lines have been read.
    pub fn get_lines_coloured(&self) -> u32 {
        self.lines_coloured
    }

    /// Whether there is anything left to read.
    pub fn finished(&self) -> bool {
        self.engine_state.is_none()
    }

    /// Reads through `line` and appends newly produced spans to `into`.
    ///
    /// A parser may read beyond the requested line because it has no range API.
    pub fn read_colours_to_line(
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
        if self.lines_coloured as usize >= target {
            return;
        }
        let Some(engine_state) = self.engine_state.as_mut() else {
            return;
        };
        // The parser may satisfy the request by reading the complete file.
        let before = into.len();
        let from = self.lines_coloured as usize;
        engine.colour(engine_state, palette, lines, from..target, into);
        self.lines_coloured += (into.len() - before) as u32;
        if self.lines_coloured as usize >= lines.len() {
            // Release parser state after the last line.
            self.engine_state = None;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::detect::Clues;
    use crate::style::{Capture, Pen, Rule, Style};

    fn palette() -> Palette {
        // Rust uses both storage and keyword scopes for `fn`.
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
            self.highlighted.read_colours_to_line(
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
        assert_eq!(case.highlighted.get_lines_coloured(), 0);
        assert!(
            case.spans.is_empty(),
            "not read yet, so nothing handed back"
        );
    }

    #[test]
    fn reaching_a_line_reads_at_least_up_to_it() {
        let mut case = rust(&["fn a() {}", "fn b() {}", "fn c() {}"]);
        case.reach(1);
        assert!(
            case.highlighted.get_lines_coloured() >= 2,
            "at least what was asked for"
        );
        assert!(!case.spans[0].is_empty(), "`fn` is a keyword");
    }

    #[test]
    fn what_is_handed_back_matches_what_was_read() {
        // The caller uses the count to place the returned spans.
        let mut case = rust(&["fn a() {}", "fn b() {}", "fn c() {}"]);
        case.reach(2);
        assert_eq!(
            case.spans.len(),
            case.highlighted.get_lines_coloured() as usize
        );
    }

    #[test]
    fn a_line_is_handed_back_once_and_only_once() {
        // Overlapping requests must not append a line twice.
        let mut case = rust(&["fn a() {}", "fn b() {}", "fn c() {}"]);
        case.reach(0);
        let after_first = case.spans.len();
        case.reach(2);
        assert_eq!(
            case.spans.len(),
            case.highlighted.get_lines_coloured() as usize,
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
        assert_eq!(
            case.highlighted.get_lines_coloured(),
            2,
            "did not go backwards"
        );
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
        // A parser may return more lines than requested.
        let mut case = rust(&["fn a() {}", "fn b() {}", "fn c() {}"]);
        case.reach(0);
        assert!(
            case.highlighted.get_lines_coloured() >= 1,
            "at least the line asked for"
        );
        assert!(
            case.highlighted.get_lines_coloured() <= case.lines.len() as u32,
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
        h.read_colours_to_line(&engine, &palette, 9_999, &[], &mut spans);
        assert_eq!(h.get_lines_coloured(), 0);
        assert!(spans.is_empty(), "nothing to hand back");
    }
}
