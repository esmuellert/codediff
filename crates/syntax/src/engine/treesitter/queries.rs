//! Compiling a grammar's query for the highlight engine.
//!
//! Compilation costs 16 ms (Rust) to 247 ms (Haskell), is indivisible, and
//! cannot be serialized ([tree-sitter#1942](https://github.com/tree-sitter/tree-sitter/issues/1942)).
//! Done on first use and kept for the life of the process.

use std::sync::OnceLock;

use tree_sitter_highlight::HighlightConfiguration;

use super::Grammar;
use super::languages::{LANGUAGES, Parser};
use crate::style::{Capture, Style};

/// The caller's capture names, and the queries compiled against them.
///
/// A query is compiled against a *list of names*, so it cannot be built until
/// the caller has said which captures it recognises — which is why the
/// compiled configurations live here and not in the [`Engine`].
///
/// Compiled on first use and kept: it is 22 ms for Rust and 36 ms for Ruby,
/// which is worth paying once and not worth paying for a language the reader
/// never opens. (An issue from 2022 reports 500 ms for Ruby; on 0.26 it is
/// 36 ms, measured.)
pub struct Palette {
    names: Vec<&'static str>,
    styles: Vec<Style>,
    configs: Vec<OnceLock<Option<HighlightConfiguration>>>,
}

impl Palette {
    pub fn new(captures: &[Capture]) -> Self {
        Self {
            names: captures.iter().map(|c| c.name).collect(),
            styles: captures.iter().map(|c| c.style).collect(),
            configs: LANGUAGES.iter().map(|_| OnceLock::new()).collect(),
        }
    }

    /// How the caller said this capture looks.
    pub(super) fn style(&self, index: usize) -> Style {
        self.styles.get(index).copied().unwrap_or(Style::PLAIN)
    }

    /// The compiled query for a language, building it on first use.
    ///
    /// Compilation is 16–180 ms per language and cannot be done ahead of time
    /// (opaque C struct, no serialization). Runs on the worker thread.
    pub(super) fn config(&self, grammar: Grammar) -> Option<&HighlightConfiguration> {
        self.configs[grammar.0]
            .get_or_init(|| build(&LANGUAGES[grammar.0], &self.names))
            .as_ref()
    }

    /// The configuration an injected language needs.
    ///
    /// What a fenced code block in Markdown, or `<script>` in HTML, resolves
    /// to. An unknown name simply gets no colour rather than failing the file.
    pub(super) fn config_named(&self, name: &str) -> Option<&HighlightConfiguration> {
        let at = LANGUAGES.iter().position(|p| p.name == name)?;
        self.config(Grammar(at))
    }

    /// How many languages compiled, for a test that would otherwise never
    /// notice a query the engine refused.
    pub fn compiled(&self) -> usize {
        (0..LANGUAGES.len())
            .filter(|n| self.config(Grammar(*n)).is_some())
            .count()
    }
}

/// Neovim metadata captures (`@spell`, `@none`, `@conceal`) that must be
/// stripped from queries. An unrecognised capture wins over recognised ones
/// and resolves to nothing, which would leave matched regions uncoloured.
const IGNORED: &[&str] = &["spell", "nospell", "conceal", "none"];

/// The query with its metadata captures taken out.
fn without_metadata(query: &str) -> String {
    let mut out = String::with_capacity(query.len());
    let mut rest = query;
    while let Some(at) = rest.find('@') {
        let (before, from) = rest.split_at(at);
        let name: String = from[1..]
            .chars()
            .take_while(|c| c.is_alphanumeric() || *c == '.' || *c == '_')
            .collect();
        if IGNORED.contains(&name.as_str()) {
            // Take the whitespace before it too, so `@comment @spell` does not
            // become `@comment ` and a lone `@spell` line does not leave a gap.
            out.push_str(before.trim_end_matches([' ', '\t']));
        } else {
            out.push_str(before);
            out.push('@');
            out.push_str(&name);
        }
        rest = &from[1 + name.len()..];
    }
    out.push_str(rest);
    out
}

fn build(parser: &Parser, names: &[&str]) -> Option<HighlightConfiguration> {
    let highlights = without_metadata(&parser.highlights.join("\n"));
    let mut config = HighlightConfiguration::new(
        (parser.language)(),
        parser.name,
        &highlights,
        parser.injections,
        parser.locals,
    )
    .ok()?;
    config.configure(names);
    Some(config)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::style::Pen;

    fn palette() -> Palette {
        Palette::new(&[
            Capture::new("keyword", Style::pen(Pen(0))),
            Capture::new("string", Style::pen(Pen(1))),
            Capture::new("comment", Style::pen(Pen(2))),
        ])
    }

    #[test]
    fn every_language_in_the_table_compiles_its_query() {
        // A query the engine refuses is dropped silently and that language
        // simply has no colour, which nothing else here would notice.
        assert_eq!(palette().compiled(), LANGUAGES.len());
    }

    #[test]
    fn metadata_captures_are_removed_and_real_ones_are_not() {
        assert_eq!(
            without_metadata("(comment) @comment @spell"),
            "(comment) @comment"
        );
        assert_eq!(without_metadata("(x) @spell"), "(x)");
        // Not a prefix match: `@spelling` is somebody's real capture.
        assert_eq!(without_metadata("(x) @spelling"), "(x) @spelling");
        assert_eq!(
            without_metadata("(x) @comment.documentation"),
            "(x) @comment.documentation"
        );
        // A predicate mentioning a capture keeps working.
        assert_eq!(
            without_metadata(r#"((x) @c (#match? @c "^/"))"#),
            r#"((x) @c (#match? @c "^/"))"#
        );
    }
}
