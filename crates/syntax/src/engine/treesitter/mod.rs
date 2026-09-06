//! Tree-sitter language selection and highlighting.
//!
//! The engine uses each grammar's highlight query and converts highlight events
//! into per-line spans.

use tree_sitter_highlight::{HighlightConfiguration, HighlightEvent, Highlighter};

use crate::detect::Clues;
use crate::style::{Span, Style, coalesce};

mod languages;
mod queries;

use languages::LANGUAGES;
pub use queries::Palette;

/// Index of a language in the parser table.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Grammar(usize);

/// Parser facade. Language handles and compiled queries live in the table and palette.
pub struct Engine;

impl Engine {
    pub fn new() -> Self {
        Self
    }

    /// Finds a parser by file name, extension, or shebang.
    ///
    /// Unknown files return `None` and can use the TextMate fallback.
    pub fn find(&self, clues: Clues<'_>) -> Option<Grammar> {
        let file_name = clues.file_name();
        let extension = clues.extension().map(str::to_ascii_lowercase);
        let shebang = clues.shebang();

        LANGUAGES
            .iter()
            .position(|p| {
                p.file_names.contains(&file_name)
                    || extension
                        .as_deref()
                        .is_some_and(|e| p.extensions.contains(&e))
                    || shebang.is_some_and(|s| p.shebangs.contains(&s))
            })
            .map(Grammar)
    }

    /// What we call this language, for tests and for a status line.
    pub fn name(&self, grammar: Grammar) -> &'static str {
        LANGUAGES[grammar.0].name
    }

    /// Highlights the complete snapshot and appends one span vector per line.
    ///
    /// The Tree-sitter highlighter has no range API.
    pub fn colour(
        &self,
        grammar: Grammar,
        palette: &Palette,
        lines: &[String],
        into: &mut Vec<Vec<Span>>,
    ) {
        let Some(config) = palette.config(grammar) else {
            // Keep the response aligned when the query is unavailable.
            into.extend(std::iter::repeat_n(Vec::new(), lines.len()));
            return;
        };
        let mut output = vec![Vec::new(); lines.len()];
        {
            // Tree-sitter consumes one contiguous source string.
            let mut source = String::with_capacity(lines.iter().map(|l| l.len() + 1).sum());
            let mut starts = Vec::with_capacity(lines.len());
            for line in lines {
                starts.push(source.len());
                source.push_str(line);
                source.push('\n');
            }
            paint(config, palette, &source, &starts, lines, &mut output);
        }
        into.append(&mut output);
    }
}

impl Default for Engine {
    fn default() -> Self {
        Self::new()
    }
}

/// Walks the engine's events into per-line spans.
fn paint(
    config: &HighlightConfiguration,
    palette: &Palette,
    source: &str,
    starts: &[usize],
    lines: &[String],
    output: &mut [Vec<Span>],
) {
    let mut highlighter = Highlighter::new();
    // Keep plain lines if highlighting fails.
    let Ok(events) = highlighter.highlight(config, source.as_bytes(), None, |name| {
        palette.config_named(name)
    }) else {
        return;
    };

    let mut open: Vec<Style> = Vec::new();
    for event in events {
        match event {
            Ok(HighlightEvent::HighlightStart(highlight)) => open.push(palette.style(highlight.0)),
            Ok(HighlightEvent::HighlightEnd) => {
                open.pop();
            }
            // The innermost capture is the most specific one.
            Ok(HighlightEvent::Source { start, end }) => {
                if let Some(style) = open.last() {
                    spread(start, end, *style, starts, lines, output);
                }
            }
            Err(_) => return,
        }
    }

    for line in output.iter_mut() {
        *line = coalesce(std::mem::take(line));
    }
    let _ = source;
}

/// Splits a multi-line byte range at line boundaries.
fn spread(
    start: usize,
    end: usize,
    style: Style,
    starts: &[usize],
    lines: &[String],
    output: &mut [Vec<Span>],
) {
    let first = starts.partition_point(|s| *s <= start).saturating_sub(1);
    for (n, line_start) in starts.iter().enumerate().skip(first) {
        if *line_start >= end {
            break;
        }
        let line_end = line_start + lines[n].len();
        let from = start.max(*line_start) - line_start;
        let to = end.min(line_end).saturating_sub(*line_start);
        if from < to {
            output[n].push(Span::new(from as u32..to as u32, style));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::style::{Capture, Pen};

    fn palette() -> Palette {
        Palette::new(&[
            Capture::new("keyword", Style::pen(Pen(0))),
            Capture::new("string", Style::pen(Pen(1))),
            Capture::new("comment", Style::pen(Pen(2))),
            Capture::new("punctuation", Style::pen(Pen(5))),
            Capture::new("type", Style::pen(Pen(3))),
            Capture::new("function", Style::pen(Pen(4))),
        ])
    }

    fn read(path: &str, source: &str) -> Vec<Vec<Span>> {
        let engine = Engine::new();
        let palette = palette();
        let lines: Vec<String> = source.lines().map(str::to_owned).collect();
        let grammar = engine
            .find(Clues::new(path, lines.first().map(String::as_str)))
            .unwrap_or_else(|| panic!("no parser claims {path}"));
        let mut out = Vec::new();
        engine.colour(grammar, &palette, &lines, &mut out);
        out
    }

    #[test]
    fn no_two_languages_claim_the_same_extension() {
        // The first match wins, so a duplicate makes one row unreachable.
        for (n, parser) in LANGUAGES.iter().enumerate() {
            for extension in parser.extensions {
                let earlier = LANGUAGES[..n]
                    .iter()
                    .find(|p| p.extensions.contains(extension));
                assert!(
                    earlier.is_none(),
                    "{extension} is claimed by both {} and {}",
                    earlier.map_or("", |p| p.name),
                    parser.name
                );
            }
        }
    }

    #[test]
    fn preparing_a_language_happens_once_and_then_costs_nothing() {
        // The compiled query is cached by the palette.
        let engine = Engine::new();
        let palette = palette();
        let lines = vec!["fn a() {}".to_owned()];

        let grammar = engine.find(Clues::new("a.rs", None)).expect("rust");
        let mut first = Vec::new();
        engine.colour(grammar, &palette, &lines, &mut first);
        assert!(!first[0].is_empty(), "`fn` is a keyword");

        let mut again = Vec::new();
        engine.colour(grammar, &palette, &lines, &mut again);
        assert_eq!(again, first, "and the same answer the second time");
    }

    #[test]
    fn a_type_in_use_position_is_coloured() {
        // The parser can classify a type in use position.
        let spans = read("a.rs", "fn f(area: Rect) {}\n");
        let at = |byte: u32| {
            spans[0]
                .iter()
                .find(|s| s.bytes.contains(&byte))
                .and_then(|s| s.style.pen)
        };
        assert_eq!(at(0), Some(Pen(0)), "`fn` is a keyword");
        assert_eq!(at(11), Some(Pen(3)), "`Rect` is a type");
    }

    #[test]
    fn one_line_at_a_time_is_what_the_caller_gets() {
        let spans = read("a.rs", "// one\nfn two() {}\n// three\n");
        assert_eq!(spans.len(), 3);
        assert!(!spans[0].is_empty() && !spans[1].is_empty() && !spans[2].is_empty());
    }

    #[test]
    fn a_construct_spanning_lines_is_cut_at_the_line_ends() {
        // One multi-line range becomes one span per line.
        let spans = read("a.rs", "/* one\n   two\n   three */\nfn f() {}\n");
        for (line, spans) in spans.iter().take(3).enumerate() {
            let covered = spans.iter().any(|s| s.style.pen == Some(Pen(2)));
            assert!(covered, "line {line} is inside the comment");
        }
        for span in spans.iter().flatten() {
            assert!(span.bytes.end <= 30, "no span runs past its own line");
        }
    }

    #[test]
    fn a_language_we_have_no_parser_for_is_refused_rather_than_guessed() {
        let engine = Engine::new();
        assert!(engine.find(Clues::new("notes.qqzz", None)).is_none());
        assert!(engine.find(Clues::new("a.rs", None)).is_some());
    }

    #[test]
    fn a_file_name_and_a_shebang_are_enough_on_their_own() {
        let engine = Engine::new();
        let by_name = engine.find(Clues::new("Gemfile", None)).expect("Gemfile");
        assert_eq!(engine.name(by_name), "ruby");
        let by_shebang = engine
            .find(Clues::new("bin/release", Some("#!/usr/bin/env python3")))
            .expect("a shebang");
        assert_eq!(engine.name(by_shebang), "python");
    }

    #[test]
    fn every_language_colours_its_comments() {
        // Ignored metadata must not suppress real comment captures.
        let engine = Engine::new();
        let palette = palette();
        for (n, parser) in LANGUAGES.iter().enumerate() {
            let Some(comment) = COMMENTS.iter().find(|(name, _)| *name == parser.name) else {
                continue;
            };
            let lines: Vec<String> = comment.1.lines().map(str::to_owned).collect();
            let mut out = Vec::new();
            engine.colour(Grammar(n), &palette, &lines, &mut out);
            assert!(
                out.iter().flatten().any(|s| s.style.pen == Some(Pen(2))),
                "{}: a comment was not coloured",
                parser.name
            );
        }
    }

    /// One commented line per language, for the test above.
    const COMMENTS: &[(&str, &str)] = &[
        ("rust", "// note\nfn a() {}\n"),
        ("python", "# note\nx = 1\n"),
        ("javascript", "// note\nlet x = 1;\n"),
        ("typescript", "// note\nlet x: number = 1;\n"),
        ("tsx", "// note\nlet x = 1;\n"),
        ("go", "// note\npackage a\n"),
        ("java", "// note\nclass A {}\n"),
        ("c", "/* note */\nint a;\n"),
        ("cpp", "// note\nint a;\n"),
        ("c_sharp", "// note\nclass A {}\n"),
        ("ruby", "# note\nx = 1\n"),
        ("php", "<?php\n// note\n$x = 1;\n"),
        ("bash", "# note\nx=1\n"),
        ("json", "// note\n{}\n"),
        ("yaml", "# note\na: 1\n"),
        ("toml", "# note\na = 1\n"),
        ("css", "/* note */\na { color: red; }\n"),
        ("html", "<!-- note -->\n<p>x</p>\n"),
        ("lua", "-- note\nlocal a = 1\n"),
        ("scala", "// note\nclass A\n"),
        ("swift", "// note\nclass A {}\n"),
        ("haskell", "-- note\nmain = print 1\n"),
        ("elixir", "# note\nx = 1\n"),
        ("nix", "# note\n{ a = 1; }\n"),
        ("sql", "-- note\nSELECT 1;\n"),
    ];
}
