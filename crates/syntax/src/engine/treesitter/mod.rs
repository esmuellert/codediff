//! The second engine: a parser, rather than regular expressions over lines.
//!
//! A TextMate grammar recognises *shapes* — a word after `fn`, a name inside a
//! parameter list — and nothing else. It cannot tell `Rect` in `area: Rect`
//! from `frame` in `frame.text`, because to a regular expression they are both
//! just words. Measured on this repository's own source, that leaves **35% of
//! identifiers with no scope at all**; `bat` on the same files leaves exactly
//! the same words uncoloured, so it is the grammar's limit and not a theme's.
//!
//! A parser knows. `Rect` is a `type_identifier` node, and the shipped
//! `highlights.scm` says `(type_identifier) @type`. The same measurement here
//! is 21%, and what remains is plain locals, which Catppuccin draws in the
//! ordinary text colour anyway.
//!
//! This does not replace [`syntect`](super::syntect). There are about
//! forty maintained grammars against two-face's 183 languages, so the TextMate
//! engine stays as the fallback and a file we have no parser for is coloured
//! exactly as it is today. See D39.
//!
//! ---
//!
//! Three things about this engine are worth knowing before reading it:
//!
//! - It is not resumable. `tree_sitter_highlight` has no range parameter;
//!   it parses the whole document every call. That would be fatal to a lazy
//!   renderer if it were slow, but it runs at ~190 000 lines a second — more
//!   than ten times `syntect` — so the whole file costs less than one of
//!   `syntect`'s capped slices, and [`Highlighted`] simply gets everything on
//!   the first ask.
//! - The queries are given, not written. Each grammar crate ships its own
//!   `highlights.scm`. There is no table of scope selectors here, and none of
//!   the TextMate precedence work that `theme::scopes` needed.
//! - We deliberately use the crates' own queries, not Neovim's. The only
//!   thing nvim-treesitter's forks add is `(identifier) @variable`, which
//!   Catppuccin paints in the ordinary text colour — no visible difference —
//!   and they use Neovim-only predicates (`#lua-match?`) that this engine
//!   *silently ignores*, which would turn `((identifier) @type (#lua-match? …))`
//!   into an unconditional `(identifier) @type` and paint every word as a
//!   type. All cost, no benefit.
//!
//! ---
//!
//! Three files, three nouns:
//!
//! ```text
//! mod.rs        this: finding a grammar, and reading a file into spans
//! languages.rs  which languages there are, and what their queries say
//! queries.rs    turning that query text into something to match with
//! ```
//!
//! [`Highlighted`]: crate::Highlighted

use tree_sitter_highlight::{HighlightConfiguration, HighlightEvent, Highlighter};

use crate::detect::Clues;
use crate::style::{Span, Style, coalesce};

mod languages;
mod queries;

use languages::LANGUAGES;
pub use queries::Palette;

/// Which language to parse a file as.
///
/// An index into [`languages::LANGUAGES`], so it can be stored beside a
/// buffer without borrowing anything.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Grammar(usize);

/// Every grammar we can parse with.
///
/// Holds nothing: a `tree_sitter::Language` is produced on demand and the
/// compiled queries live in the [`Palette`], because a query is compiled
/// against the caller's list of capture names.
pub struct Engine;

impl Engine {
    pub fn new() -> Self {
        Self
    }

    /// Which language parses this file, if we have one.
    ///
    /// Name, then extension, then shebang — most certain first, the same order
    /// the TextMate engine uses. A file nothing here claims is *not* an error:
    /// the caller falls through to `syntect`, which knows 183 languages, so an
    /// incomplete table costs nothing but the better answer.
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

    /// Colours the whole file at once.
    ///
    /// There is no smaller unit available: `tree_sitter_highlight` has no
    /// range parameter and re-parses the document on every call, so asking for
    /// part of a file would cost the same as asking for all of it and would
    /// have to be paid again. Since the whole file is around ten times cheaper
    /// than the TextMate engine's, that is a better deal than it sounds.
    ///
    /// Appends one entry per line, so the caller's line count is the file's.
    pub fn read(
        &self,
        grammar: Grammar,
        palette: &Palette,
        lines: &[String],
        into: &mut Vec<Vec<Span>>,
    ) {
        let Some(config) = palette.config(grammar) else {
            // No usable query, so no colour is ever coming. Fill the lines in
            // blank rather than say nothing: a caller told nothing would wait
            // for ever.
            into.extend(std::iter::repeat_n(Vec::new(), lines.len()));
            return;
        };
        let mut read = vec![Vec::new(); lines.len()];
        {
            // One allocation of the file, dropped on the way out. The engine
            // wants contiguous bytes and we hold lines; there is no way round
            // it that does not cost more than it saves.
            let mut source = String::with_capacity(lines.iter().map(|l| l.len() + 1).sum());
            let mut starts = Vec::with_capacity(lines.len());
            for line in lines {
                starts.push(source.len());
                source.push_str(line);
                source.push('\n');
            }
            paint(config, palette, &source, &starts, lines, &mut read);
        }
        into.append(&mut read);
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
    read: &mut [Vec<Span>],
) {
    let mut highlighter = Highlighter::new();
    // A file the engine gives up on keeps its lines and loses its colour,
    // which is what every other failure here does too.
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
            // Only the innermost claim counts. A capture nested inside another
            // is the more specific statement about that text, which is the
            // same rule TextMate precedence arrives at the long way round.
            Ok(HighlightEvent::Source { start, end }) => {
                if let Some(style) = open.last() {
                    spread(start, end, *style, starts, lines, read);
                }
            }
            Err(_) => return,
        }
    }

    for line in read.iter_mut() {
        *line = coalesce(std::mem::take(line));
    }
    let _ = source;
}

/// Cuts one byte range of the file into per-line spans.
///
/// A block comment or a multi-line string arrives as a single range covering
/// several lines, and a [`Span`] is an offset into *one* line, so it has to be
/// divided at the line boundaries.
fn spread(
    start: usize,
    end: usize,
    style: Style,
    starts: &[usize],
    lines: &[String],
    read: &mut [Vec<Span>],
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
            read[n].push(Span::new(from as u32..to as u32, style));
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
        engine.read(grammar, &palette, &lines, &mut out);
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
        // Building the index from a query is 16 ms for Rust and 250 ms for
        // Haskell, once per process. It happens here, on whatever thread the
        // painter runs on, and the second file in a language is free.
        let engine = Engine::new();
        let palette = palette();
        let lines = vec!["fn a() {}".to_owned()];

        let grammar = engine.find(Clues::new("a.rs", None)).expect("rust");
        let mut first = Vec::new();
        engine.read(grammar, &palette, &lines, &mut first);
        assert!(!first[0].is_empty(), "`fn` is a keyword");

        let mut again = Vec::new();
        engine.read(grammar, &palette, &lines, &mut again);
        assert_eq!(again, first, "and the same answer the second time");
    }

    #[test]
    fn a_type_in_use_position_is_coloured() {
        // The whole reason this engine exists. A TextMate grammar reports no
        // scope at all for `Rect` here.
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
        // A block comment arrives as one range covering three lines; a span
        // is an offset into one line, so it has to be divided.
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
        // The failure `@spell` caused: a grammar whose comment rule is
        // suppressed has no comments at all, and nothing else here notices,
        // because every other construct still works.
        let engine = Engine::new();
        let palette = palette();
        for (n, parser) in LANGUAGES.iter().enumerate() {
            let Some(comment) = COMMENTS.iter().find(|(name, _)| *name == parser.name) else {
                continue;
            };
            let lines: Vec<String> = comment.1.lines().map(str::to_owned).collect();
            let mut out = Vec::new();
            engine.read(Grammar(n), &palette, &lines, &mut out);
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
