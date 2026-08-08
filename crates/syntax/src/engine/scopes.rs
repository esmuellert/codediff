//! Which TextMate scope path maps to which [`Group`].
//!
//! One shared table — which scopes are keywords is a fact about TextMate, not
//! a theme choice. Precedence is the engine's (deeper + more specific wins).
//!
//! Every selector is tested against real source in seventeen languages; a typo
//! that matches nothing fails the build.

use crate::group::Group;
use crate::style::Style;

/// One entry of the scope table.
#[derive(Debug, Clone, Copy)]
pub struct Scope {
    pub selector: &'static str,
    pub group: Group,
    bold: bool,
    italic: bool,
    underline: bool,
}

impl Scope {
    /// The emphasis this entry carries, with no pen in it yet.
    ///
    /// The pen is added by [`palette`](super::palette), which is the only
    /// place that knows what number this entry has.
    pub(super) const fn emphasis(&self) -> Style {
        Style {
            pen: None,
            bold: self.bold,
            italic: self.italic,
            underline: self.underline,
            strikethrough: false,
        }
    }
}

const fn scope(selector: &'static str, group: Group) -> Scope {
    Scope {
        selector,
        group,
        bold: false,
        italic: false,
        underline: false,
    }
}

impl Scope {
    const fn bold(self) -> Self {
        Self { bold: true, ..self }
    }
    const fn italic(self) -> Self {
        Self {
            italic: true,
            ..self
        }
    }
    const fn underline(self) -> Self {
        Self {
            underline: true,
            ..self
        }
    }
}

/// Every scope rule handed to the engine.
///
/// A [`Pen`] is an index into this table, so entries must not be reordered.
pub const SCOPES: &[Scope] = {
    use Group as T;
    &[
        // --- comments ---
        scope("comment", T::Comment).italic(),
        // The `//` belongs to the comment, not to the punctuation. Without
        // this every comment in the file starts with two grey characters.
        scope("punctuation.definition.comment", T::Comment).italic(),
        // --- strings ---
        scope("string", T::String),
        // Its quotes, and only its quotes. A bare `punctuation.definition
        // .string` would claim the delimiters of a regular expression and of
        // a character literal too, because those are scoped as strings —
        // naming the enclosing scope is the only way to tell them apart, and
        // the engine scores the longer path higher, which is what makes the
        // three rules resolve in the order written.
        scope("string punctuation.definition.string", T::String),
        // A character literal, only in the languages that have a character
        // type. `string.quoted.single` alone would repaint every ordinary
        // Python and JavaScript string, which use the same quotes for the
        // same thing.
        scope(
            "string.quoted.single.c, string.quoted.single.c++, \
             string.quoted.single.rust, string.quoted.single.java, \
             string.quoted.single.cs, string.quoted.single.go",
            T::Character,
        ),
        scope(
            "string.quoted.single.c punctuation.definition.string, \
             string.quoted.single.c++ punctuation.definition.string, \
             string.quoted.single.rust punctuation.definition.string, \
             string.quoted.single.java punctuation.definition.string, \
             string.quoted.single.cs punctuation.definition.string, \
             string.quoted.single.go punctuation.definition.string",
            T::Character,
        ),
        scope("constant.character.escape", T::Escape),
        scope("constant.other.placeholder", T::Escape),
        scope("string.regexp", T::Regexp),
        // The delimiters of a regular expression are part of it, and are
        // scoped as string punctuation, which the rule above would otherwise
        // claim. The enclosing scope is the only thing that can tell them
        // apart, which is what a descendant selector is for.
        scope("string.regexp punctuation.definition.string", T::Regexp),
        scope("string.regexp keyword", T::Regexp),
        scope("string.regexp constant", T::Regexp),
        // --- numbers and other constants ---
        scope("constant", T::Constant),
        scope("entity.name.constant", T::Constant),
        // --- keywords ---
        scope("keyword", T::Keyword),
        scope("keyword.control", T::Keyword),
        scope("keyword.control.import", T::Keyword),
        scope("keyword.operator", T::Operator),
        scope("keyword.declaration", T::Keyword),
        // `storage` is every type-ish reserved word: `let`, `int`, `u32`,
        // `struct`, `func`, `static`, `class`. All keywords, and both
        // references agree — VS Code's `dark_plus` gives `storage.type` the
        // same blue as `keyword.control`, and Catppuccin sends
        // `@type.builtin` to Mauve. What earns the *type* colour below is a
        // type's name, which is `entity.name.type` and `support.type`.
        //
        // One rule, not eight. Every `storage.type.*` a grammar spells —
        // Rust's `.struct` and `.impl`, Go's `.keyword.func`, Java's
        // `.primitive` — resolves here, and a row per grammar would say
        // nothing this does not. Add one back the day a theme wants `struct`
        // to differ from `int`.
        scope("storage", T::Keyword),
        scope("meta.preprocessor", T::Preprocessor),
        // --- names ---
        scope("entity.name.type", T::Type),
        scope("entity.name.class", T::Type),
        scope("entity.name.struct", T::Type),
        scope("entity.name.enum", T::Type),
        scope("entity.name.union", T::Type),
        scope("entity.name.trait", T::Type),
        scope("entity.other.inherited-class", T::Type),
        scope("support.type", T::Type),
        scope("support.class", T::Type),
        scope("entity.name.function", T::Function),
        scope("variable.function", T::Function),
        // Not `meta.function-call`: it spans the arguments as well as the
        // name, so it would colour `a` and `b` in `f(a, b)` as functions.
        // VS Code's `dark_plus` leaves it out for the same reason.
        scope("support.function", T::Library),
        scope("support.macro", T::Library),
        scope("entity.name.namespace", T::Namespace),
        scope("entity.name.module", T::Namespace),
        scope("entity.name.label", T::Label),
        scope("entity.name.other.anchor", T::Label),
        scope("punctuation.definition.anchor", T::Label),
        scope("punctuation.definition.alias", T::Label),
        scope("variable", T::Variable),
        scope("variable.language", T::Builtin),
        scope("support.variable", T::Builtin),
        scope("variable.parameter", T::Parameter),
        scope("variable.other.member", T::Property),
        scope("variable.other.property", T::Property),
        scope("variable.object.property", T::Property),
        // --- attributes, annotations, decorators ---
        scope("entity.other.attribute-name", T::Attribute),
        scope("meta.annotation", T::Attribute),
        scope("meta.decorator", T::Attribute),
        scope("variable.annotation", T::Attribute),
        scope("punctuation.definition.annotation", T::Attribute),
        // The name in `@sealed` is scoped as an ordinary variable; only the
        // enclosing decorator says otherwise.
        scope("meta.annotation variable", T::Attribute),
        scope("meta.decorator variable", T::Attribute),
        // --- punctuation ---
        scope("punctuation", T::Punctuation),
        scope("punctuation.separator", T::Punctuation),
        scope("punctuation.terminator", T::Punctuation),
        // --- interpolation: code inside a string is code ---
        //
        // Without these, the whole of `` `hello ${user.name}` `` is one shade
        // of green. `meta.template.expression` exists for exactly this, and
        // leaving it out is the single most visible thing a small theme gets
        // wrong.
        scope("meta.interpolation", T::Variable),
        scope("meta.template.expression", T::Variable),
        scope("punctuation.section.interpolation", T::Escape),
        scope("punctuation.definition.template-expression", T::Escape),
        // --- data formats, where the key is the structure ---
        scope("entity.name.tag", T::Tag),
        scope("support.type.property-name", T::Property),
        scope("meta.mapping.key", T::Property),
        // JSON spells a key as a string and YAML spells it as a tag; in both
        // the enclosing scope is the only thing that says it is a key.
        scope("meta.mapping.key string", T::Property),
        scope(
            "meta.mapping.key string punctuation.definition.string",
            T::Property,
        ),
        // YAML has no `meta.mapping.key` at all: it spells a key as a tag
        // inside an unquoted string, so the language has to be named. This is
        // the one place where `entity.name.tag` does not mean markup.
        scope("entity.name.tag.yaml", T::Property),
        // --- markup, because a reviewer reads a great deal of it ---
        scope("markup.heading", T::Heading).bold(),
        scope("punctuation.definition.heading", T::Heading).bold(),
        scope("entity.name.section", T::Heading).bold(),
        scope("markup.bold", T::Emphasis).bold(),
        scope("punctuation.definition.bold", T::Emphasis).bold(),
        scope("markup.italic", T::Emphasis).italic(),
        scope("punctuation.definition.italic", T::Emphasis).italic(),
        scope("markup.underline", T::Emphasis).underline(),
        scope("markup.underline.link", T::Link).underline(),
        scope("markup.raw", T::Raw),
        scope("punctuation.definition.raw", T::Raw),
        scope("markup.list", T::List),
        scope("punctuation.definition.list_item", T::List),
        scope("markup.quote", T::Quote).italic(),
        scope("punctuation.definition.blockquote", T::Quote).italic(),
        scope("meta.link", T::Reference),
        scope("markup.inserted", T::Inserted),
        scope("markup.deleted", T::Deleted),
        // The `+` and `-` belong to the line they mark, and are scoped as
        // punctuation inside it — the same shape as a comment's `//`.
        scope("markup.inserted punctuation", T::Inserted),
        scope("markup.deleted punctuation", T::Deleted),
        // --- what the grammar thinks is broken ---
        scope("invalid", T::Invalid),
        scope("invalid.deprecated", T::Invalid).italic(),
    ]
};

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn no_two_scopes_name_the_same_selector() {
        // Two entries for one selector means the later silently wins, and one
        // of the two colours is unreachable.
        for (n, s) in SCOPES.iter().enumerate() {
            assert!(
                !SCOPES[..n]
                    .iter()
                    .any(|earlier| earlier.selector == s.selector),
                "{} appears twice",
                s.selector
            );
        }
    }

    #[test]
    fn every_group_is_claimed_by_some_scope() {
        // A group nothing reaches is a colour nobody can ever see. The compiler
        // cannot say so, because an unused struct field is legal.
        for expected in Group::ALL {
            assert!(
                SCOPES.iter().any(|s| s.group == expected),
                "no scope produces {}",
                expected.name()
            );
        }
    }
}
