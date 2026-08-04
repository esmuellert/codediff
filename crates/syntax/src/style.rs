//! What a highlighter says about text, and what a caller says about colour.
//!
//! The two travel in opposite directions. A [`Rule`] goes **in**: `ui` says
//! "anything matching `keyword.control` is mauve and italic". A [`Span`] comes
//! **out**: "bytes 4..9 of this line wear that style".
//!
//! Neither names an engine, and neither is a colour this crate chose.

use std::ops::Range;

/// Which of the caller's pens a run of text is written with.
///
/// **Not a colour — the number of one.** `ui` hands in rules saying "anything
/// matching `keyword` is pen 4" and gets back spans saying "bytes 0..2 are
/// pen 4". What pen 4 looks like stays in the theme, which is what this crate
/// promises never to know.
///
/// Three things fall out of the indirection, and each is the reason for it:
///
/// - a terminal with no 24-bit colour can be given `Color::Indexed`, which an
///   `Rgb` here could not express — and that is the entire reason the `basic`
///   theme exists;
/// - changing theme does not invalidate a single span, because no span
///   mentions a colour;
/// - the scope-to-pen table is one shared constant instead of one per theme,
///   since which scopes are keywords is a fact about TextMate, not taste.
///
/// VS Code does exactly this: its token metadata packs an index into a
/// `ColorMap` rather than a colour.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Pen(pub u16);

/// How a run of text looks.
///
/// A pen and four independent flags — no background, and no second colour.
/// That is the whole composition rule: a diff owns the background of every
/// line it touches, and syntax may only tint the letters on top. Give syntax a
/// background and a changed line stops looking changed. See the crate README.
///
/// The flags are separate rather than an enum because a theme sets them
/// independently: `markup.bold` is bold with no pen of its own, and
/// `markup.italic` is both.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Style {
    pub pen: Option<Pen>,
    pub bold: bool,
    pub italic: bool,
    pub underline: bool,
    pub strikethrough: bool,
}

impl Style {
    /// A style that changes nothing, which is what an unmatched scope gets.
    pub const PLAIN: Style = Style {
        pen: None,
        bold: false,
        italic: false,
        underline: false,
        strikethrough: false,
    };

    pub const fn pen(pen: Pen) -> Self {
        Self {
            pen: Some(pen),
            ..Self::PLAIN
        }
    }

    pub const fn italic(self) -> Self {
        Self {
            italic: true,
            ..self
        }
    }

    pub const fn bold(self) -> Self {
        Self { bold: true, ..self }
    }

    /// Whether this style would do anything at all.
    pub fn is_plain(&self) -> bool {
        *self == Self::PLAIN
    }
}

/// One theme rule: which scopes it claims, and how they look.
///
/// `selector` is a TextMate scope selector — `keyword.control`, or
/// `string.quoted`, or the contextual `source.css entity.other.attribute-name`.
/// Matching is by **prefix over a dotted path**, so `keyword` claims
/// `keyword.control.rust` unless a longer rule claims it more specifically.
///
/// A path rather than one of a dozen category names, because a real theme
/// needs the distinction: `keyword` and `keyword.control` are different
/// colours in every theme worth shipping, and `meta.template.expression`
/// exists to put interpolated code back to the colour of code. See D36.
///
/// `'static` because the table of them is a constant: which scopes are
/// keywords is a fact about TextMate, not a choice a theme makes.
#[derive(Debug, Clone, Copy)]
pub struct Rule {
    pub selector: &'static str,
    pub style: Style,
}

impl Rule {
    pub const fn new(selector: &'static str, style: Style) -> Self {
        Self { selector, style }
    }
}

/// A run of one line that shares a style.
///
/// Byte offsets into that line, half-open, so the range slices the line
/// directly and cannot land inside a character. Byte offsets rather than
/// columns on purpose: a tab is one byte and several columns, and the renderer
/// already knows how to map one to the other. `delta` expands tabs *before*
/// highlighting because it works in strings; we do not have to, because we
/// work in ranges.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Span {
    pub bytes: Range<u32>,
    pub style: Style,
}

impl Span {
    pub fn new(bytes: Range<u32>, style: Style) -> Self {
        Self { bytes, style }
    }
}

/// Merges neighbouring spans that wear the same style.
///
/// A grammar happily reports six adjacent runs of plain text; a renderer would
/// rather have one. Also drops runs that ask for nothing, since the caller
/// already paints an unstyled line.
pub fn coalesce(spans: Vec<Span>) -> Vec<Span> {
    let mut out: Vec<Span> = Vec::with_capacity(spans.len());
    for span in spans {
        if span.bytes.is_empty() || span.style.is_plain() {
            continue;
        }
        match out.last_mut() {
            Some(last) if last.style == span.style && last.bytes.end == span.bytes.start => {
                last.bytes.end = span.bytes.end;
            }
            _ => out.push(span),
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    const RED: Style = Style::pen(Pen(1));
    const BLUE: Style = Style::pen(Pen(2));

    #[test]
    fn touching_runs_of_one_style_become_one() {
        let merged = coalesce(vec![
            Span::new(0..3, RED),
            Span::new(3..7, RED),
            Span::new(7..9, BLUE),
        ]);
        assert_eq!(merged, vec![Span::new(0..7, RED), Span::new(7..9, BLUE)]);
    }

    #[test]
    fn a_gap_keeps_them_apart() {
        let merged = coalesce(vec![Span::new(0..3, RED), Span::new(5..7, RED)]);
        assert_eq!(merged.len(), 2);
    }

    #[test]
    fn runs_that_ask_for_nothing_are_dropped() {
        // The caller has already painted the line in the ordinary colour, so a
        // span saying "ordinary" is work with no effect.
        assert!(coalesce(vec![Span::new(0..9, Style::PLAIN)]).is_empty());
        assert!(coalesce(vec![Span::new(4..4, RED)]).is_empty(), "empty");
    }

    #[test]
    fn a_style_can_carry_a_flag_and_no_pen() {
        // `markup.bold` is exactly this, and dropping it would lose the rule.
        let bold = Style::PLAIN.bold();
        assert!(!bold.is_plain());
        assert_eq!(bold.pen, None);
    }
}
