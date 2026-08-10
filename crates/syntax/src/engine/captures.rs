//! Which tree-sitter capture maps to which [`Group`].
//!
//! The twin of [`scopes`](super::scopes) for the parser engine. Simpler
//! because captures have no precedence rules — the engine matches by longest
//! prefix, so only names that need a different answer from their prefix appear.

use crate::group::Group;

/// One entry, before it is given a pen.
pub struct Name {
    pub name: &'static str,
    pub group: Group,
}

const fn name(name: &'static str, group: Group) -> Name {
    Name { name, group }
}

/// Every capture we recognise.
///
/// The vocabulary is the union of what the twenty-five grammars in the table
/// actually write, which is not quite tree-sitter's published standard list
/// and not quite Neovim's — several grammars still use the pre-2024 names
/// (`@parameter`, `@field`, `@method`, `@conditional`), so both spellings are
/// here. A name nothing uses costs nothing.
pub const NAMES: &[Name] = {
    use Group as T;
    &[
        // --- the shape every language has ---
        // Upright, for the reason given beside the matcher's `comment`.
        name("comment", T::Comment),
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn no_two_captures_name_the_same_thing() {
        // Two entries for one name means the later silently wins.
        for (n, entry) in NAMES.iter().enumerate() {
            assert!(
                !NAMES[..n].iter().any(|earlier| earlier.name == entry.name),
                "{} appears twice",
                entry.name
            );
        }
    }
}
