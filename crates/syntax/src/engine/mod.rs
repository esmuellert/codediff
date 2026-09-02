//! Chooses Tree-sitter when available and TextMate otherwise.

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

/// A grammar owned by either engine.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Grammar {
    Tree(treesitter::Grammar),
    TextMate(syntect::Grammar),
}

/// State needed to continue colouring a file.
pub enum EngineState {
    Tree(treesitter::Grammar),
    TextMate(Box<syntect::SyntectState>),
}

/// The syntax group assigned to a pen.
pub fn group(pen: Pen) -> Option<Group> {
    let n = pen.0 as usize;
    match scopes::SCOPES.get(n) {
        Some(scope) => Some(scope.group),
        None => captures::NAMES
            .get(n - scopes::SCOPES.len())
            .map(|c| c.group),
    }
}

/// TextMate rules, numbered from zero.
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

/// Tree-sitter captures, numbered after the TextMate rules.
fn captures() -> Vec<Capture> {
    let base = scopes::SCOPES.len() as u16;
    captures::NAMES
        .iter()
        .enumerate()
        .map(|(n, entry)| Capture::new(entry.name, Style::pen(Pen(base + n as u16))))
        .collect()
}

/// The same pens represented for both engines.
pub struct Palette {
    textmate: syntect::Palette,
    trees: treesitter::Palette,
}

impl Palette {
    /// Builds both engine palettes from the shared pen numbering.
    pub fn new() -> Self {
        Self {
            textmate: syntect::Palette::new(&rules()),
            trees: treesitter::Palette::new(&captures()),
        }
    }

    /// Builds a palette from caller-supplied tables.
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

    /// Chooses a grammar, preferring Tree-sitter.
    pub fn find(&self, clues: Clues<'_>, _lines: usize) -> Option<Grammar> {
        if let Some(grammar) = self.trees.find(clues) {
            return Some(Grammar::Tree(grammar));
        }
        self.textmate.find(clues).map(Grammar::TextMate)
    }

    /// Chooses a TextMate grammar without consulting Tree-sitter.
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

    /// Appends coloured spans; Tree-sitter may read beyond `rows`.
    pub fn colour(
        &self,
        engine_state: &mut EngineState,
        palette: &Palette,
        lines: &[String],
        rows: Range<usize>,
        into: &mut Vec<Vec<Span>>,
    ) {
        match engine_state {
            EngineState::Tree(grammar) => self.trees.colour(*grammar, &palette.trees, lines, into),
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

    #[test]
    fn no_comment_rule_asks_for_a_font_style() {
        let styles = rules()
            .into_iter()
            .map(|rule| (rule.selector, rule.style))
            .chain(captures().into_iter().map(|c| (c.name, c.style)));
        for (name, style) in styles {
            if style.pen.and_then(group) != Some(Group::Comment) {
                continue;
            }
            assert_eq!(
                style,
                Style::pen(style.pen.expect("just read")),
                "{name} gives a comment more than a colour"
            );
        }
    }

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
        let engine = Engine::new();
        let grammar = engine
            .find(Clues::new("Makefile", None), 10)
            .expect("two-face knows make");
        assert!(matches!(grammar, Grammar::TextMate(_)));
    }

    #[test]
    fn a_very_long_file_is_parsed_too() {
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
        let engine = Engine::new();
        let palette = palette();
        let lines = vec!["fn a() {}".to_owned()];
        let read = |grammar: Grammar| {
            let mut engine_state = engine.start(grammar, &palette);
            let mut out = Vec::new();
            engine.colour(&mut engine_state, &palette, &lines, 0..1, &mut out);
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
