//! Which tree-sitter capture wears which pen.
//!
//! ---
//!
//! The parser's half of the theme, and the exact twin of [`scopes`]: that file
//! maps TextMate scope paths, this one maps the capture names a grammar's own
//! `highlights.scm` uses. Both land on the same [`Token`]s, which is what lets
//! one theme serve two engines and a file look the same whichever read it.
//!
//! **Shorter than [`scopes`], and that is the point.** A capture is what the
//! grammar's author already decided; there is no precedence to arrange, no
//! contextual selector to get right, and no typo to hunt — a name that matches
//! nothing simply never appears. Most of the work in `scopes` was fighting
//! TextMate's matching rules, and none of it exists here.
//!
//! Matching is by **longest dotted prefix**, done by the engine: a query that
//! captures `@keyword.function` finds `keyword` here if `keyword.function` is
//! not listed. So only the names that need a *different* answer from their
//! prefix appear below, plus the prefixes themselves.
//!
//! [`scopes`]: super::scopes

use syntax::{Capture, Pen, Style};

use super::code::Token;

/// One entry, before it is given a pen.
pub struct Name {
    pub name: &'static str,
    pub token: Token,
    italic: bool,
}

const fn name(name: &'static str, token: Token) -> Name {
    Name {
        name,
        token,
        italic: false,
    }
}

impl Name {
    const fn italic(self) -> Self {
        Self {
            italic: true,
            ..self
        }
    }
}

/// The first pen this table uses.
///
/// Pens are one space shared by both engines, so the parser's names start
/// where the matcher's selectors stop. That is what lets `Code` resolve a pen
/// without knowing which engine produced it.
pub const BASE: u16 = super::scopes::SCOPES.len() as u16;

/// Every capture we recognise.
///
/// The vocabulary is the union of what the twenty-five grammars in the table
/// actually write, which is not quite tree-sitter's published standard list
/// and not quite Neovim's — several grammars still use the pre-2024 names
/// (`@parameter`, `@field`, `@method`, `@conditional`), so both spellings are
/// here. A name nothing uses costs nothing.
pub const NAMES: &[Name] = {
    use Token as T;
    &[
        // --- the shape every language has ---
        name("comment", T::Comment).italic(),
        name("string", T::String),
        name("string.escape", T::Escape),
        name("string.regex", T::Regexp),
        name("string.regexp", T::Regexp),
        // JavaScript spells a regular expression this way, and Elixir a sigil.
        name("string.special", T::Regexp),
        name("string.special.key", T::Property),
        name("string.special.path", T::Link),
        name("string.special.uri", T::Link),
        name("string.special.symbol", T::Constant),
        name("escape", T::Escape),
        name("character", T::Character),
        name("character.special", T::Escape),
        name("number", T::Constant),
        name("float", T::Constant),
        name("boolean", T::Constant),
        name("constant", T::Constant),
        name("constant.macro", T::Library),
        // --- keywords ---
        name("keyword", T::Keyword),
        name("keyword.operator", T::Operator),
        name("keyword.directive", T::Preprocessor),
        name("preproc", T::Preprocessor),
        name("operator", T::Operator),
        // The pre-2024 spellings, still shipped by several grammars.
        name("conditional", T::Keyword),
        name("repeat", T::Keyword),
        name("exception", T::Keyword),
        name("include", T::Keyword),
        name("import", T::Keyword),
        name("storageclass", T::Keyword),
        // --- types ---
        name("type", T::Type),
        // A built-in type is a reserved word — `u32`, `int`, `string`. Both
        // references agree: VS Code gives `storage.type` its keyword colour
        // and Catppuccin sends `@type.builtin` to Mauve. The *name* of a type
        // is what earns the type colour. Same decision as `scopes`.
        name("type.builtin", T::Keyword),
        name("type.qualifier", T::Keyword),
        name("type.definition", T::Type),
        name("constructor", T::Type),
        // --- names ---
        name("function", T::Function),
        name("function.builtin", T::Library),
        name("function.macro", T::Library),
        name("function.special", T::Library),
        name("method", T::Function),
        name("variable", T::Variable),
        name("variable.builtin", T::Builtin),
        name("variable.parameter", T::Parameter),
        name("variable.member", T::Property),
        name("parameter", T::Parameter),
        name("property", T::Property),
        name("field", T::Property),
        name("attribute", T::Attribute),
        name("module", T::Namespace),
        name("namespace", T::Namespace),
        name("label", T::Label),
        // --- punctuation ---
        name("punctuation", T::Punctuation),
        name("punctuation.special", T::Escape),
        name("delimiter", T::Punctuation),
        // --- markup and data ---
        name("tag", T::Tag),
        name("tag.error", T::Invalid),
        // CSS spells its at-rules as captures of their own.
        name("keyframes", T::Keyword),
        name("media", T::Keyword),
        name("supports", T::Keyword),
        name("charset", T::Keyword),
    ]
};

/// Which token a pen from this table names, if it is one.
pub fn token(pen: Pen) -> Option<Token> {
    let at = pen.0.checked_sub(BASE)? as usize;
    NAMES.get(at).map(|n| n.token)
}

/// The table, as `syntax` wants it.
pub fn captures() -> Vec<Capture> {
    NAMES
        .iter()
        .enumerate()
        .map(|(n, entry)| {
            let mut style = Style::pen(Pen(BASE + n as u16));
            if entry.italic {
                style = style.italic();
            }
            Capture::new(entry.name, style)
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::theme::Theme;

    #[test]
    fn every_capture_carries_its_own_position() {
        for (n, capture) in captures().iter().enumerate() {
            assert_eq!(
                capture.style.pen,
                Some(Pen(BASE + n as u16)),
                "{}",
                capture.name
            );
            assert_eq!(capture.name, NAMES[n].name);
        }
    }

    #[test]
    fn the_two_tables_do_not_share_a_pen() {
        // One pen space across both engines, so the ranges must not overlap or
        // a parsed file would be coloured by the matcher's table.
        for (n, _) in NAMES.iter().enumerate() {
            let pen = Pen(BASE + n as u16);
            assert!(super::super::scopes::token(pen).is_none(), "{pen:?}");
            assert!(token(pen).is_some());
        }
        for (n, _) in super::super::scopes::SCOPES.iter().enumerate() {
            assert!(token(Pen(n as u16)).is_none());
        }
    }

    #[test]
    fn no_two_captures_name_the_same_thing() {
        for (n, entry) in NAMES.iter().enumerate() {
            assert!(
                !NAMES[..n].iter().any(|e| e.name == entry.name),
                "{} appears twice",
                entry.name
            );
        }
    }

    #[test]
    fn a_pen_resolves_to_the_colour_its_capture_asked_for() {
        let code = Theme::DARK.code;
        for (n, entry) in NAMES.iter().enumerate() {
            assert_eq!(
                code.pen(Some(Pen(BASE + n as u16))),
                Some(code.colour(entry.token)),
                "{}",
                entry.name
            );
        }
    }
}
