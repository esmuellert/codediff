//! TextMate scope selectors mapped to syntax groups.
//!
//! The table is shared by all themes and follows TextMate specificity.

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
    /// The engine layer adds the pen after the table's position is known.
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
/// A syntax pen is an index into this table, so entries must not be reordered.
pub const SCOPES: &[Scope] = {
    use Group as T;
    &[
        // --- comments ---
        scope("comment", T::Comment),
        // Comment punctuation uses the comment group too.
        scope("punctuation.definition.comment", T::Comment),
        // --- strings ---
        scope("string", T::String),
        // Restrict string punctuation to string scopes.
        scope("string punctuation.definition.string", T::String),
        // Character literals need language-qualified selectors.
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
        // Regex punctuation must stay in the regex group.
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
        // Reserved words use the keyword group, including storage scopes.
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
        // Function calls must not include their arguments.
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
        // The enclosing decorator supplies the attribute group.
        scope("meta.annotation variable", T::Attribute),
        scope("meta.decorator variable", T::Attribute),
        // --- punctuation ---
        scope("punctuation", T::Punctuation),
        scope("punctuation.separator", T::Punctuation),
        scope("punctuation.terminator", T::Punctuation),
        // --- interpolation: expressions use code groups ---
        scope("meta.interpolation", T::Variable),
        scope("meta.template.expression", T::Variable),
        scope("punctuation.section.interpolation", T::Escape),
        scope("punctuation.definition.template-expression", T::Escape),
        // --- data formats, where the key is the structure ---
        scope("entity.name.tag", T::Tag),
        scope("support.type.property-name", T::Property),
        scope("meta.mapping.key", T::Property),
        // The enclosing mapping scope distinguishes keys from string values.
        scope("meta.mapping.key string", T::Property),
        scope(
            "meta.mapping.key string punctuation.definition.string",
            T::Property,
        ),
        // YAML keys use a language-qualified tag scope.
        scope("entity.name.tag.yaml", T::Property),
        // --- markup ---
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
        // Patch markers use the surrounding inserted/deleted group.
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
        // Duplicate selectors make one entry unreachable.
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
        // Every group must have at least one selector.
        for expected in Group::ALL {
            assert!(
                SCOPES.iter().any(|s| s.group == expected),
                "no scope produces {}",
                expected.name()
            );
        }
    }
}
