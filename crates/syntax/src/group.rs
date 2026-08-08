//! What a stretch of text is, in words both engines answer in.
//!
//! A syntax group (Vim's `:help group-name`): what text *is*, not what it
//! looks like. The matcher's `comment.line.double-slash.rust` and the parser's
//! `comment` both map to [`Group::Comment`], making the two engines
//! interchangeable per file.

/// What a stretch of text is, for the purpose of colouring it.
///
/// The groups Catppuccin distinguishes — a superset of VS Code's `dark_plus`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Group {
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

impl Group {
    /// Every token, once. Order is not meaningful; completeness is.
    pub const ALL: [Group; 31] = [
        Group::Comment,
        Group::String,
        Group::Character,
        Group::Escape,
        Group::Regexp,
        Group::Constant,
        Group::Keyword,
        Group::Operator,
        Group::Preprocessor,
        Group::Type,
        Group::Function,
        Group::Library,
        Group::Variable,
        Group::Builtin,
        Group::Parameter,
        Group::Property,
        Group::Namespace,
        Group::Label,
        Group::Punctuation,
        Group::Tag,
        Group::Attribute,
        Group::Invalid,
        Group::Heading,
        Group::Link,
        Group::Reference,
        Group::Raw,
        Group::List,
        Group::Quote,
        Group::Emphasis,
        Group::Inserted,
        Group::Deleted,
    ];

    /// What to call it in a message.
    pub const fn name(self) -> &'static str {
        match self {
            Group::Comment => "comment",
            Group::String => "string",
            Group::Character => "character",
            Group::Escape => "escape",
            Group::Regexp => "regexp",
            Group::Constant => "constant",
            Group::Keyword => "keyword",
            Group::Operator => "operator",
            Group::Preprocessor => "preprocessor",
            Group::Type => "type",
            Group::Function => "function",
            Group::Library => "library",
            Group::Variable => "variable",
            Group::Builtin => "builtin",
            Group::Parameter => "parameter",
            Group::Property => "property",
            Group::Namespace => "namespace",
            Group::Label => "label",
            Group::Punctuation => "punctuation",
            Group::Tag => "tag",
            Group::Attribute => "attribute",
            Group::Invalid => "invalid",
            Group::Heading => "heading",
            Group::Link => "link",
            Group::Reference => "reference",
            Group::Raw => "raw",
            Group::List => "list",
            Group::Quote => "quote",
            Group::Emphasis => "emphasis",
            Group::Inserted => "inserted",
            Group::Deleted => "deleted",
        }
    }
}
