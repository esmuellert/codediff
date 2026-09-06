//! Syntect/TextMate syntax highlighting.
//!
//! This module owns grammar lookup, parser state, and scope matching.

use syntect::highlighting::{
    Color, FontStyle, HighlightState, Highlighter, RangedHighlightIterator, ScopeSelectors,
    StyleModifier, Theme, ThemeItem, ThemeSettings,
};
use syntect::parsing::{ParseState, ScopeStack, SyntaxSet};

use crate::detect::Clues;
use crate::limits;
use crate::style::{Pen, Rule, Span, Style, coalesce};

/// All TextMate grammars loaded by the process.
pub struct Engine {
    syntaxes: SyntaxSet,
}

/// Index of a TextMate grammar in the syntax table.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Grammar(usize);

/// Which scope wears which pen, in the form the engine matches against.
pub struct Palette {
    theme: Theme,
}

/// Parser and theme state carried from one line to the next.
pub struct SyntectState {
    parse: ParseState,
    highlight: HighlightState,
    /// Reused buffer containing the current line and a newline.
    buffer: String,
}

impl Engine {
    pub fn new() -> Self {
        Self {
            syntaxes: two_face::syntax::extra_newlines(),
        }
    }

    /// Finds a grammar by known name, extension, file name, or shebang.
    ///
    /// Unknown files return `None` and are rendered as plain text.
    pub fn find(&self, clues: Clues<'_>) -> Option<Grammar> {
        let by_name = clues
            .well_known()
            .and_then(|name| self.syntaxes.find_syntax_by_name(name));
        let by_extension = || {
            clues
                .extension()
                .and_then(|ext| self.syntaxes.find_syntax_by_extension(ext))
        };
        // Some extension tables also contain complete file names.
        let by_file_name = || self.syntaxes.find_syntax_by_extension(clues.file_name());
        let by_shebang = || {
            clues
                .shebang()
                .and_then(|interpreter| self.syntaxes.find_syntax_by_token(interpreter))
        };
        let found = by_name
            .or_else(by_extension)
            .or_else(by_file_name)
            .or_else(by_shebang)?;
        self.syntaxes
            .syntaxes()
            .iter()
            .position(|syntax| std::ptr::eq(syntax, found))
            .map(Grammar)
    }

    /// Returns the grammar name.
    pub fn name(&self, grammar: Grammar) -> &str {
        &self.syntaxes.syntaxes()[grammar.0].name
    }

    /// Starts parser state for a file.
    pub fn start(&self, grammar: Grammar, palette: &Palette) -> SyntectState {
        let syntax = &self.syntaxes.syntaxes()[grammar.0];
        let highlighter = Highlighter::new(&palette.theme);
        SyntectState {
            parse: ParseState::new(syntax),
            highlight: HighlightState::new(&highlighter, ScopeStack::new()),
            buffer: String::new(),
        }
    }

    /// Reads lines in order and appends their spans.
    pub fn read(
        &self,
        engine_state: &mut SyntectState,
        palette: &Palette,
        lines: &[String],
        into: &mut Vec<Vec<Span>>,
    ) {
        let matcher = Highlighter::new(&palette.theme);
        for line in lines {
            let spans = self.read_line(engine_state, &matcher, line);
            into.push(spans);
        }
    }

    /// Reads one more line, and says how it is coloured.
    ///
    /// Reads in order from the first line so multiline grammar state is kept.
    /// Lines over the colour limit are parsed but return no spans.
    fn read_line(
        &self,
        engine_state: &mut SyntectState,
        matcher: &Highlighter<'_>,
        line: &str,
    ) -> Vec<Span> {
        engine_state.buffer.clear();
        engine_state.buffer.push_str(line);
        engine_state.buffer.push('\n');

        let Ok(ops) = engine_state
            .parse
            .parse_line(&engine_state.buffer, &self.syntaxes)
        else {
            // A grammar that failed on one line has not failed on the file.
            return Vec::new();
        };
        if !limits::worth_colouring(line) {
            // Keep the parse state, drop the colour.
            return Vec::new();
        }

        let spans = RangedHighlightIterator::new(
            &mut engine_state.highlight,
            &ops,
            &engine_state.buffer,
            matcher,
        )
        .map(|(style, _, range)| {
            // The newline we added is not part of the line the caller holds.
            let end = range.end.min(line.len());
            Span::new(range.start as u32..end as u32, convert(style))
        })
        .collect();
        coalesce(spans)
    }
}

impl Default for Engine {
    fn default() -> Self {
        Self::new()
    }
}

impl Palette {
    /// Builds a TextMate theme from the caller's scope rules.
    ///
    /// Selectors the engine cannot parse are ignored.
    pub fn new(rules: &[Rule]) -> Self {
        let scopes = rules
            .iter()
            .filter_map(|rule| {
                Some(ThemeItem {
                    scope: rule.selector.parse::<ScopeSelectors>().ok()?,
                    style: modifier(rule.style),
                })
            })
            .collect();
        Self {
            theme: Theme {
                name: None,
                author: None,
                // Transparency marks scopes that no rule claimed.
                settings: ThemeSettings {
                    foreground: Some(UNCLAIMED),
                    ..ThemeSettings::default()
                },
                scopes,
            },
        }
    }

    /// How many rules the engine accepted.
    pub fn rules(&self) -> usize {
        self.theme.scopes.len()
    }
}

/// Our style, as the engine wants it.
fn modifier(style: Style) -> StyleModifier {
    let mut font = FontStyle::empty();
    font.set(FontStyle::BOLD, style.bold);
    font.set(FontStyle::ITALIC, style.italic);
    font.set(FontStyle::UNDERLINE, style.underline);
    StyleModifier {
        foreground: style.pen.map(encode),
        // Diff backgrounds are applied by the UI.
        background: None,
        font_style: Some(font),
    }
}

/// Encodes a pen in the colour field carried by syntect.
///
/// Alpha distinguishes encoded pens from an unclaimed scope.
const fn encode(Pen(n): Pen) -> Color {
    Color {
        r: (n >> 8) as u8,
        g: n as u8,
        b: 0,
        a: 0xff,
    }
}

const fn decode(colour: Color) -> Option<Pen> {
    if colour.a == 0 {
        return None;
    }
    Some(Pen(((colour.r as u16) << 8) | colour.g as u16))
}

/// Transparent marker for an unclaimed scope.
const UNCLAIMED: Color = Color {
    r: 0,
    g: 0,
    b: 0,
    a: 0,
};

/// The engine's style, as we want it.
fn convert(style: syntect::highlighting::Style) -> Style {
    Style {
        pen: decode(style.foreground),
        bold: style.font_style.contains(FontStyle::BOLD),
        italic: style.font_style.contains(FontStyle::ITALIC),
        underline: style.font_style.contains(FontStyle::UNDERLINE),
        strikethrough: false,
    }
}
