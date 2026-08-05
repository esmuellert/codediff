//! What colour a theme gives each part of a piece of code.
//!
//! ---
//!
//! Taste, and only taste. What a stretch of text *is* — that `keyword.control`
//! is a keyword — is a fact about an engine, so [`syntax::Group`] and the two
//! tables that produce it live in that crate. This file says what a keyword
//! *looks like*, which every theme answers differently.
//!
//! Between them sits [`syntax::Pen`], a number. `syntax` hands back "bytes
//! 4..9 are pen 12" and answers which group pen 12 is; only a [`Code`] knows
//! that a keyword is mauve. That is why a terminal with no 24-bit colour can
//! still be highlighted, and why changing theme does not re-read a line.
//!
//! **A [`Code`] holds `Color`, not `Style`.** Not a detail: syntax may only
//! tint the letters, because a diff owns the background of every line it
//! touches and a syntax background would hide which lines changed. Storing a
//! colour rather than a style means a theme *cannot* express the mistake.

use ratatui::style::Color;
use syntax::{Group, Pen};

use super::catppuccin::Palette;
use super::colour::Rgb;

/// The colour a theme gives each [`Group`].
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
    /// An exhaustive match rather than an array, so adding a [`Group`] fails
    /// to compile until every theme has said what it looks like.
    pub const fn colour(&self, token: Group) -> Color {
        match token {
            Group::Comment => self.comment,
            Group::String => self.string,
            Group::Character => self.character,
            Group::Escape => self.escape,
            Group::Regexp => self.regexp,
            Group::Constant => self.constant,
            Group::Keyword => self.keyword,
            Group::Operator => self.operator,
            Group::Preprocessor => self.preprocessor,
            Group::Type => self.kind,
            Group::Function => self.function,
            Group::Library => self.library,
            Group::Variable => self.variable,
            Group::Builtin => self.builtin,
            Group::Parameter => self.parameter,
            Group::Property => self.property,
            Group::Namespace => self.namespace,
            Group::Label => self.label,
            Group::Punctuation => self.punctuation,
            Group::Tag => self.tag,
            Group::Attribute => self.attribute,
            Group::Invalid => self.invalid,
            Group::Heading => self.heading,
            Group::Link => self.link,
            Group::Reference => self.reference,
            Group::Raw => self.raw,
            Group::List => self.list,
            Group::Quote => self.quote,
            Group::Emphasis => self.emphasis,
            Group::Inserted => self.inserted,
            Group::Deleted => self.deleted,
        }
    }

    /// The colour a span asks for, or nothing if no rule claimed it.
    ///
    /// **Either engine's pen.** The two tables share one numbering — scope
    /// selectors first, capture names after — so this resolves a span without
    /// knowing which engine produced it, which is the whole reason a diff of a
    /// parsed file and a matched file can wear one theme.
    ///
    /// A pen out of range is not an error worth failing a frame over — it can
    /// only mean spans outlived the palette that made them — so it draws
    /// plainly, which is what an unhighlighted line does anyway.
    pub fn pen(&self, pen: Option<Pen>) -> Option<Color> {
        Some(self.colour(syntax::group(pen?)?))
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
    fn a_pen_resolves_to_the_colour_its_group_asked_for() {
        // Which pen is which group is `syntax`'s to say; this only checks that
        // a theme answers for whatever it says.
        let code = Theme::DARK.code;
        for rule in syntax::rules() {
            let pen = rule.style.pen.expect("every rule carries its pen");
            let group = syntax::group(pen).expect("and every pen names a group");
            assert_eq!(
                code.pen(Some(pen)),
                Some(code.colour(group)),
                "{}",
                rule.selector
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
            for token in Group::ALL {
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
