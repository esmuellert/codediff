//! What a piece of code is, and what colour a theme gives it.
//!
//! ---
//!
//! Two halves that must not be confused. [`scopes`] says *what* a stretch of
//! text is — that `keyword.control` is a keyword — which is a fact about
//! TextMate and is therefore one table shared by every theme. [`Code`] says
//! what a keyword *looks like*, which is taste, and every theme fills it in.
//!
//! Between them sits [`syntax::Pen`], a number. `syntax` is handed the scopes
//! and hands back "bytes 4..9 are pen 12"; only this file knows that pen 12 is
//! a keyword, and only a [`Code`] knows that a keyword is mauve. That is why a
//! terminal with no 24-bit colour can still be highlighted, and why changing
//! theme does not re-read a single line.
//!
//! **A [`Code`] holds `Color`, not `Style`.** Not a detail: syntax may only
//! tint the letters, because a diff owns the background of every line it
//! touches and a syntax background would hide which lines changed. Storing a
//! colour rather than a style means a theme *cannot* express the mistake.

use ratatui::style::Color;
use syntax::Pen;

use super::catppuccin::Palette;
use super::colour::Rgb;
use super::scopes;

/// What a stretch of text is, for the purpose of colouring it.
///
/// The roles Catppuccin parts, which is a superset of the ones VS Code's
/// `dark_plus` parts. Fewer would be simpler and wrong: a theme that cannot
/// tell a parameter from a field, or a regular expression from a string, is
/// not the theme people installed.
///
/// Source: `catppuccin/nvim`, `lua/catppuccin/groups/{syntax,treesitter}.lua`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Token {
    /// A comment or a docstring.
    Comment,
    /// Text between quotes.
    String,
    /// A single-character literal: `'c'`.
    Character,
    /// `\n` inside a string, which is not the string.
    Escape,
    /// A regular expression, which is not an ordinary string either.
    Regexp,
    /// A number, a boolean, `nil`, an enum member.
    Constant,
    /// `if`, `return`, `pub` — the words the language reserves.
    Keyword,
    /// `+`, `=>`, `&&`.
    Operator,
    /// `#include`, `#[cfg]`, a shebang.
    Preprocessor,
    /// A type, class, struct or trait.
    Type,
    /// A function or method, defined or called.
    Function,
    /// One the runtime provides: `println!`, `console.log`, `printf`.
    Library,
    /// An ordinary variable.
    Variable,
    /// One the language defines for you: `self`, `this`, `super`.
    Builtin,
    /// A parameter of a function, where the grammar says so.
    Parameter,
    /// A field, a member, a key in a data format.
    Property,
    /// A module or namespace.
    Namespace,
    /// A `goto` label, a `case`, a YAML anchor.
    Label,
    /// Brackets, commas, semicolons.
    Punctuation,
    /// A tag in markup: `<div>`.
    Tag,
    /// An attribute, decorator or annotation.
    Attribute,
    /// Something the grammar believes is wrong.
    Invalid,

    // --- markup, because a reviewer reads a great deal of it ---
    /// `# Heading`.
    Heading,
    /// A URL.
    Link,
    /// The visible text of a link, and a footnote reference.
    Reference,
    /// Inline code, and a fenced block.
    Raw,
    /// A bullet or a number starting a list item.
    List,
    /// A block quote.
    Quote,
    /// Bold or italic text. Carries a colour as well as the flag, because
    /// Catppuccin gives emphasis one.
    Emphasis,
    /// A line a `.patch` file adds, read as content rather than as our own
    /// diff — reviewing a patch is reviewing a file like any other.
    Inserted,
    /// A line it removes.
    Deleted,
}

impl Token {
    /// Every token, once. Order is not meaningful; completeness is.
    pub const ALL: [Token; 31] = [
        Token::Comment,
        Token::String,
        Token::Character,
        Token::Escape,
        Token::Regexp,
        Token::Constant,
        Token::Keyword,
        Token::Operator,
        Token::Preprocessor,
        Token::Type,
        Token::Function,
        Token::Library,
        Token::Variable,
        Token::Builtin,
        Token::Parameter,
        Token::Property,
        Token::Namespace,
        Token::Label,
        Token::Punctuation,
        Token::Tag,
        Token::Attribute,
        Token::Invalid,
        Token::Heading,
        Token::Link,
        Token::Reference,
        Token::Raw,
        Token::List,
        Token::Quote,
        Token::Emphasis,
        Token::Inserted,
        Token::Deleted,
    ];

    /// What to call it in a message.
    pub const fn name(self) -> &'static str {
        match self {
            Token::Comment => "comment",
            Token::String => "string",
            Token::Character => "character",
            Token::Escape => "escape",
            Token::Regexp => "regexp",
            Token::Constant => "constant",
            Token::Keyword => "keyword",
            Token::Operator => "operator",
            Token::Preprocessor => "preprocessor",
            Token::Type => "type",
            Token::Function => "function",
            Token::Library => "library",
            Token::Variable => "variable",
            Token::Builtin => "builtin",
            Token::Parameter => "parameter",
            Token::Property => "property",
            Token::Namespace => "namespace",
            Token::Label => "label",
            Token::Punctuation => "punctuation",
            Token::Tag => "tag",
            Token::Attribute => "attribute",
            Token::Invalid => "invalid",
            Token::Heading => "heading",
            Token::Link => "link",
            Token::Reference => "reference",
            Token::Raw => "raw",
            Token::List => "list",
            Token::Quote => "quote",
            Token::Emphasis => "emphasis",
            Token::Inserted => "inserted",
            Token::Deleted => "deleted",
        }
    }
}

/// The colour a theme gives each [`Token`].
///
/// Colours, not styles — see the module note. Bold and italic arrive from the
/// scope table instead, because they are structural: a heading is bold in
/// every theme, and a theme that made it plain would be wrong rather than
/// different.
#[derive(Debug, Clone, Copy)]
pub struct Code {
    pub comment: Color,
    pub string: Color,
    pub character: Color,
    pub escape: Color,
    pub regexp: Color,
    pub constant: Color,
    pub keyword: Color,
    pub operator: Color,
    pub preprocessor: Color,
    pub kind: Color,
    pub function: Color,
    pub library: Color,
    pub variable: Color,
    pub builtin: Color,
    pub parameter: Color,
    pub property: Color,
    pub namespace: Color,
    pub label: Color,
    pub punctuation: Color,
    pub tag: Color,
    pub attribute: Color,
    pub invalid: Color,
    pub heading: Color,
    pub link: Color,
    pub reference: Color,
    pub raw: Color,
    pub list: Color,
    pub quote: Color,
    pub emphasis: Color,
    pub inserted: Color,
    pub deleted: Color,
}

impl Code {
    /// The colour of one token.
    ///
    /// An exhaustive match rather than an array, so adding a [`Token`] fails
    /// to compile until every theme has said what it looks like.
    pub const fn colour(&self, token: Token) -> Color {
        match token {
            Token::Comment => self.comment,
            Token::String => self.string,
            Token::Character => self.character,
            Token::Escape => self.escape,
            Token::Regexp => self.regexp,
            Token::Constant => self.constant,
            Token::Keyword => self.keyword,
            Token::Operator => self.operator,
            Token::Preprocessor => self.preprocessor,
            Token::Type => self.kind,
            Token::Function => self.function,
            Token::Library => self.library,
            Token::Variable => self.variable,
            Token::Builtin => self.builtin,
            Token::Parameter => self.parameter,
            Token::Property => self.property,
            Token::Namespace => self.namespace,
            Token::Label => self.label,
            Token::Punctuation => self.punctuation,
            Token::Tag => self.tag,
            Token::Attribute => self.attribute,
            Token::Invalid => self.invalid,
            Token::Heading => self.heading,
            Token::Link => self.link,
            Token::Reference => self.reference,
            Token::Raw => self.raw,
            Token::List => self.list,
            Token::Quote => self.quote,
            Token::Emphasis => self.emphasis,
            Token::Inserted => self.inserted,
            Token::Deleted => self.deleted,
        }
    }

    /// The colour a span asks for, or nothing if no rule claimed it.
    ///
    /// A pen out of range is not an error worth failing a frame over — it can
    /// only mean spans outlived the palette that made them — so it draws
    /// plainly, which is what an unhighlighted line does anyway.
    pub fn pen(&self, pen: Option<Pen>) -> Option<Color> {
        Some(self.colour(scopes::token(pen?)?))
    }
}

/// Catppuccin's own mapping, which is the same for all four flavours — only
/// the values differ.
///
/// Every line below is `catppuccin/nvim`'s. Where its treesitter table and its
/// base table disagree, the treesitter one wins, because that is the one that
/// runs on the languages people read.
pub const fn catppuccin(p: Palette) -> Code {
    const fn c(Rgb(r, g, b): Rgb) -> Color {
        Color::Rgb(r, g, b)
    }
    Code {
        comment: c(p.overlay2),
        string: c(p.green),
        character: c(p.teal),
        escape: c(p.pink),
        regexp: c(p.pink),
        constant: c(p.peach),
        keyword: c(p.mauve),
        operator: c(p.sky),
        preprocessor: c(p.pink),
        kind: c(p.yellow),
        function: c(p.blue),
        library: c(p.peach),
        variable: c(p.text),
        builtin: c(p.red),
        parameter: c(p.maroon),
        property: c(p.lavender),
        namespace: c(p.yellow),
        label: c(p.sapphire),
        punctuation: c(p.overlay2),
        tag: c(p.blue),
        attribute: c(p.yellow),
        invalid: c(p.red),
        heading: c(p.blue),
        link: c(p.blue),
        reference: c(p.lavender),
        raw: c(p.green),
        list: c(p.teal),
        quote: c(p.pink),
        emphasis: c(p.red),
        inserted: c(p.green),
        deleted: c(p.red),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::theme::Theme;

    #[test]
    fn a_pen_resolves_to_the_colour_its_scope_asked_for() {
        let code = Theme::DARK.code;
        for (n, token) in scopes::SCOPES.iter().map(|s| s.token).enumerate() {
            assert_eq!(
                code.pen(Some(Pen(n as u16))),
                Some(code.colour(token)),
                "{}",
                scopes::SCOPES[n].selector
            );
        }
        assert_eq!(code.pen(None), None);
        assert_eq!(code.pen(Some(Pen(9999))), None, "outlived its palette");
    }

    #[test]
    fn every_theme_gives_every_token_a_colour_that_is_not_the_background() {
        // A theme that resolved a token to its own background would have
        // written a rule that erases text.
        for theme in Theme::ALL {
            for token in Token::ALL {
                let colour = theme.code.colour(token);
                if colour == Color::Reset {
                    // Not a colour: "whatever this terminal uses for text".
                    // It equals `normal.bg`, which is also `Reset`, but the
                    // two mean opposite ends of the terminal's own contrast —
                    // `basic` is built entirely on that, and comparing the
                    // enum values here would read it backwards.
                    continue;
                }
                assert_ne!(
                    Some(colour),
                    theme.normal.bg,
                    "{}: {} is invisible",
                    theme.name,
                    token.name()
                );
            }
        }
    }
}
