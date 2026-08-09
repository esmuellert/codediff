//! The choice between two engines.
//!
//! `lint-arch` refuses engine names outside this directory.
//!
//! - [`treesitter`] parses (knows types/functions, 25 languages)
//! - [`syntect`] matches regexes (183 languages, resumable)
//!
//! One engine per file: parse where we have a grammar, match otherwise.

pub mod captures;
pub mod scopes;
mod syntect;
mod treesitter;

use std::ops::Range;

use crate::detect::Clues;
use crate::group::Group;
use crate::style::{Capture, Pen, Rule, Span, Style};

/// Every grammar we have, of either kind.
pub struct Engine {
    textmate: syntect::Engine,
    trees: treesitter::Engine,
}

/// Which grammar reads a file, and therefore which engine.
///
/// An index either way, so it can be stored beside the buffer that needs it
/// without borrowing the engine.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Grammar {
    Tree(treesitter::Grammar),
    TextMate(syntect::Grammar),
}

/// How far through a file an engine has read.
///
/// The parser carries nothing between calls — it does the whole file at once —
/// so its variant is only the grammar it has yet to use. The matcher's state
/// is the reason it can stop and resume at all.
pub enum EngineState {
    Tree(treesitter::Grammar),
    TextMate(Box<syntect::SyntectState>),
}

/// Which syntax group a pen names, whichever engine produced it.
///
/// The inverse of the numbering in [`Palette::new`], and deliberately beside
/// it. Everything above asks this one question and gets one answer, which is
/// what "two engines look like one" actually means.
pub fn group(pen: Pen) -> Option<Group> {
    let n = pen.0 as usize;
    match scopes::SCOPES.get(n) {
        Some(scope) => Some(scope.group),
        None => captures::NAMES
            .get(n - scopes::SCOPES.len())
            .map(|c| c.group),
    }
}

/// The matcher's half, numbered from zero.
///
/// Public because a test may want to give the matcher its rules and nothing
/// else, to prove every selector claims something real.
pub fn rules() -> Vec<Rule> {
    scopes::SCOPES
        .iter()
        .enumerate()
        .map(|(n, scope)| {
            Rule::new(
                scope.selector,
                Style {
                    pen: Some(Pen(n as u16)),
                    ..scope.emphasis()
                },
            )
        })
        .collect()
}

/// The parser's half, numbered from where the matcher's stops.
fn captures() -> Vec<Capture> {
    let base = scopes::SCOPES.len() as u16;
    captures::NAMES
        .iter()
        .enumerate()
        .map(|(n, entry)| {
            Capture::new(
                entry.name,
                Style {
                    pen: Some(Pen(base + n as u16)),
                    ..entry.emphasis()
                },
            )
        })
        .collect()
}

/// The caller's colours, in the form each engine matches against.
///
/// Both halves, because which engine reads a file is not known when the
/// palette is built, and because a [`Pen`](crate::Pen) means the same thing
/// whichever engine produced it — that is what lets the two share one theme.
pub struct Palette {
    textmate: syntect::Palette,
    trees: treesitter::Palette,
}

impl Palette {
    /// The colours both engines answer in.
    ///
    /// The only place a pen is given a number. Both tables are numbered
    /// here, one after the other, and [`group`] reads them back with the same
    /// arithmetic ten lines away. Numbering assigned in two files is an
    /// agreement two files have to keep; numbered here it is one function that
    /// cannot disagree with itself.
    pub fn new() -> Self {
        Self {
            textmate: syntect::Palette::new(&rules()),
            trees: treesitter::Palette::new(&captures()),
        }
    }

    /// Built from tables a caller supplies rather than our own.
    ///
    /// For tests that want a palette of two rules, so a failure names the rule
    /// that failed instead of one of three hundred.
    pub fn from_tables(rules: &[Rule], captures: &[Capture]) -> Self {
        Self {
            textmate: syntect::Palette::new(rules),
            trees: treesitter::Palette::new(captures),
        }
    }

    /// How many TextMate rules the engine accepted.
    pub fn rules(&self) -> usize {
        self.textmate.rules()
    }

    /// How many languages compiled their query.
    pub fn compiled(&self) -> usize {
        self.trees.compiled()
    }
}

impl Engine {
    pub fn new() -> Self {
        Self {
            textmate: syntect::Engine::new(),
            trees: treesitter::Engine::new(),
        }
    }

    /// Which grammar reads this file, if any does.
    ///
    /// A parser is preferred wherever one exists: it answers a question the
    /// matcher cannot, and on a long file it is the faster of the two by an
    /// order of magnitude. It once was not preferred above a size, because a
    /// whole-file parse would have held a frame — colouring happens on its own
    /// thread now, so there is no frame to hold. See D41.
    ///
    /// `lines` is unused today and kept because the choice is the seam's to
    /// make: a future engine may well have an opinion about size.
    pub fn find(&self, clues: Clues<'_>, _lines: usize) -> Option<Grammar> {
        if let Some(grammar) = self.trees.find(clues) {
            return Some(Grammar::Tree(grammar));
        }
        self.textmate.find(clues).map(Grammar::TextMate)
    }

    /// Which TextMate grammar reads this file, ignoring the parser.
    ///
    /// For the tests that are about *scope selectors* — which only that engine
    /// matches — and for nothing else. A reader always gets the seam's answer;
    /// this exists so `ui`'s scope table can be checked against the engine
    /// that uses it, on languages the parser also happens to know.
    pub fn find_textmate(&self, clues: Clues<'_>) -> Option<Grammar> {
        self.textmate.find(clues).map(Grammar::TextMate)
    }

    /// What the engine calls this grammar, for tests and for a status line.
    pub fn name(&self, grammar: Grammar) -> &str {
        match grammar {
            Grammar::Tree(g) => self.trees.name(g),
            Grammar::TextMate(g) => self.textmate.name(g),
        }
    }

    /// Begins engine_state a file from its first line.
    pub fn start(&self, grammar: Grammar, palette: &Palette) -> EngineState {
        match grammar {
            Grammar::Tree(g) => EngineState::Tree(g),
            Grammar::TextMate(g) => {
                EngineState::TextMate(Box::new(self.textmate.start(g, &palette.textmate)))
            }
        }
    }

    /// Reads the lines in `want`, appending the spans for each.
    ///
    /// The range is a request, not a promise. The matcher honours it, which
    /// is what lets a frame stop halfway through a long file. The parser has no
    /// range API at all, so it reads everything and the caller gets more than
    /// it asked for — which is why [`Highlighted`] checks how far it actually
    /// got rather than assuming.
    ///
    /// [`Highlighted`]: crate::Highlighted
    pub fn read(
        &self,
        engine_state: &mut EngineState,
        palette: &Palette,
        lines: &[String],
        rows: Range<usize>,
        into: &mut Vec<Vec<Span>>,
    ) {
        match engine_state {
            EngineState::Tree(grammar) => self.trees.read(*grammar, &palette.trees, lines, into),
            EngineState::TextMate(state) => {
                self.textmate
                    .read(state, &palette.textmate, &lines[rows], into);
            }
        }
    }
}

impl Default for Engine {
    fn default() -> Self {
        Self::new()
    }
}

impl Default for Palette {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::style::{Pen, Style};

    // --- the numbering ---------------------------------------------------
    //
    // Both tables are numbered by one function above, so these cannot fail
    // the way they once could, when each table numbered itself and a third
    // file read them back. They stay because the arithmetic is still
    // arithmetic, and because a reader wants to see it stated.

    #[test]
    fn every_rule_carries_its_own_position() {
        for (n, rule) in rules().iter().enumerate() {
            assert_eq!(rule.style.pen, Some(Pen(n as u16)), "{}", rule.selector);
            assert_eq!(rule.selector, scopes::SCOPES[n].selector);
        }
    }

    #[test]
    fn every_capture_is_numbered_after_the_last_rule() {
        let base = scopes::SCOPES.len() as u16;
        for (n, capture) in captures().iter().enumerate() {
            assert_eq!(
                capture.style.pen,
                Some(Pen(base + n as u16)),
                "{}",
                capture.name
            );
        }
    }

    #[test]
    fn the_two_tables_do_not_share_a_pen() {
        // The whole reason one function numbers both. A shared pen would mean
        // a word coloured as whatever the other engine calls that number.
        let mut seen = std::collections::HashSet::new();
        for pen in rules()
            .iter()
            .filter_map(|r| r.style.pen)
            .chain(captures().iter().filter_map(|c| c.style.pen))
        {
            assert!(seen.insert(pen.0), "{pen:?} is used twice");
        }
    }

    #[test]
    fn every_pen_either_table_hands_out_names_a_group() {
        for (pen, expected) in rules()
            .iter()
            .filter_map(|r| r.style.pen)
            .zip(scopes::SCOPES.iter().map(|s| s.group))
            .chain(
                captures()
                    .iter()
                    .filter_map(|c| c.style.pen)
                    .zip(captures::NAMES.iter().map(|c| c.group)),
            )
        {
            assert_eq!(group(pen), Some(expected), "{pen:?}");
        }
    }

    #[test]
    fn a_pen_from_no_table_names_nothing() {
        assert_eq!(group(Pen(9_999)), None);
    }

    // --- the seam --------------------------------------------------------

    fn palette() -> Palette {
        Palette::from_tables(
            &[
                Rule::new("keyword", Style::pen(Pen(0))),
                Rule::new("storage", Style::pen(Pen(0))),
            ],
            &[Capture::new("keyword", Style::pen(Pen(0)))],
        )
    }

    #[test]
    fn a_language_with_a_parser_is_parsed() {
        let engine = Engine::new();
        let grammar = engine.find(Clues::new("a.rs", None), 10).expect("rust");
        assert!(matches!(grammar, Grammar::Tree(_)));
    }

    #[test]
    fn a_language_without_one_falls_back_to_the_matcher() {
        // Neither is wrong; the point is that the file is still coloured.
        let engine = Engine::new();
        let grammar = engine
            .find(Clues::new("Makefile", None), 10)
            .expect("two-face knows make");
        assert!(matches!(grammar, Grammar::TextMate(_)));
    }

    #[test]
    fn a_very_long_file_is_parsed_too() {
        // It once was not: a whole-file parse would have held a frame, so
        // above a size the matcher was used because it could stop halfway.
        // Colouring is on its own thread now, so the faster engine wins
        // outright — and on a file this long the parser is ten times faster,
        // which is precisely where that matters most. See D41.
        let engine = Engine::new();
        let long = engine.find(Clues::new("a.rs", None), 500_000).unwrap();
        assert!(matches!(long, Grammar::Tree(_)));
    }

    #[test]
    fn a_language_neither_engine_knows_is_refused() {
        let engine = Engine::new();
        assert!(engine.find(Clues::new("notes.qqzz", None), 10).is_none());
    }

    #[test]
    fn both_engines_answer_in_the_same_currency() {
        // The property the whole hybrid rests on: a `Pen` means the same thing
        // whichever engine produced it, so one theme serves both.
        let engine = Engine::new();
        let palette = palette();
        let lines = vec!["fn a() {}".to_owned()];
        let read = |grammar: Grammar| {
            let mut engine_state = engine.start(grammar, &palette);
            let mut out = Vec::new();
            engine.read(&mut engine_state, &palette, &lines, 0..1, &mut out);
            out
        };

        let parsed = read(engine.find(Clues::new("a.rs", None), 1).unwrap());
        let matched = read(Grammar::TextMate(
            engine.textmate.find(Clues::new("a.rs", None)).unwrap(),
        ));
        for out in [&parsed, &matched] {
            assert!(
                out[0].iter().any(|s| s.style.pen == Some(Pen(0))),
                "both call `fn` a keyword"
            );
        }
    }
}
