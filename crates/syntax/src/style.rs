//! Types exchanged by syntax engines and their caller.
//!
//! Rules and captures configure an engine. Spans report styled byte ranges.
//! Pens identify theme entries without storing terminal colours here.

use std::ops::Range;

/// Index into the caller's theme table.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Pen(pub u16);

/// Syntax style for a span. Backgrounds belong to the diff renderer.
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

/// TextMate selector and style.
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

/// Tree-sitter capture name and style.
#[derive(Debug, Clone, Copy)]
pub struct Capture {
    pub name: &'static str,
    pub style: Style,
}

impl Capture {
    pub const fn new(name: &'static str, style: Style) -> Self {
        Self { name, style }
    }
}

/// Half-open byte range and syntax style for one line.
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

/// Merges adjacent equal spans and removes empty or plain spans.
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
        // Plain spans add no information after the base style is applied.
        assert!(coalesce(vec![Span::new(0..9, Style::PLAIN)]).is_empty());
        assert!(coalesce(vec![Span::new(4..4, RED)]).is_empty(), "empty");
    }

    #[test]
    fn a_style_can_carry_a_flag_and_no_pen() {
        // Modifiers can be present without a pen.
        let bold = Style::PLAIN.bold();
        assert!(!bold.is_plain());
        assert_eq!(bold.pen, None);
    }
}
