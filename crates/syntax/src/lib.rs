#![doc = include_str!("../README.md")]
//!
//! ---
//!
//! Admission criterion: does this identify what a piece of text *is* — a
//! keyword, a string, a comment? Never what colour it should be: the token
//! kinds below are deliberately abstract so that `ui` owns the mapping to
//! colour and this crate can be swapped for a different highlighter without
//! touching a theme.
//!
//! This crate performs no IO. Real highlighting arrives at S11; until then
//! [`Plain`] returns nothing, which every caller must already handle because
//! highlighting is asynchronous and the first frame paints before it lands.

/// What a run of text is, syntactically.
///
/// Normalised on purpose: a highlighter that distinguishes forty token types
/// maps them onto these, so a theme has a fixed set to colour and adding a
/// language cannot add a colour.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Token {
    Keyword,
    /// A function, method or macro name.
    Function,
    Type,
    /// A variable, field or parameter.
    Variable,
    String,
    Number,
    Comment,
    /// Operators, brackets, semicolons.
    Punctuation,
    /// An annotation or decorator.
    Attribute,
    /// Text with no particular meaning.
    Plain,
}

/// A run of one line coloured as one token.
///
/// Byte offsets into that line, half-open, so the range slices the line
/// directly and cannot land inside a character.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Span {
    pub bytes: std::ops::Range<u32>,
    pub token: Token,
}

/// Something that can say what a file's text means.
pub trait Highlighter {
    /// Spans for one line, in order and not overlapping.
    ///
    /// `path` is what decides the language; the highlighter is not told the
    /// file's whole content because a renderer asks about one visible line at a
    /// time.
    fn line(&self, path: &str, line: &str) -> Vec<Span>;
}

/// A highlighter that finds nothing.
///
/// Not a placeholder to be removed: syntax highlighting is slow enough to run
/// off the render path, so every renderer must already cope with having no
/// spans yet. This is that state, made explicit and testable.
#[derive(Debug, Clone, Copy, Default)]
pub struct Plain;

impl Highlighter for Plain {
    fn line(&self, _path: &str, _line: &str) -> Vec<Span> {
        Vec::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_plain_highlighter_finds_nothing() {
        assert!(Plain.line("a.rs", "fn main() {}").is_empty());
    }
}
