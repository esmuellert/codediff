//! The syntect (TextMate regex) engine.
//!
//! `lint-arch` refuses `syntect` outside this file. Theme matching (scope
//! precedence) is syntect's own — we supply the colour choices as [`Rule`]s.

use syntect::highlighting::{
    Color, FontStyle, HighlightState, Highlighter, RangedHighlightIterator, ScopeSelectors,
    StyleModifier, Theme, ThemeItem, ThemeSettings,
};
use syntect::parsing::{ParseState, ScopeStack, SyntaxSet};

use crate::detect::Clues;
use crate::limits;
use crate::style::{Pen, Rule, Span, Style, coalesce};

/// Every grammar we can load, loaded once.
///
/// Two-face's set rather than syntect's own: it is `bat`'s, which carries the
/// languages people actually diff — TypeScript, TOML, Dockerfile — that the
/// default set omits.
pub struct Engine {
    syntaxes: SyntaxSet,
}

/// Which grammar to colour a file with.
///
/// An index rather than a reference, so it can be stored beside the buffer
/// that needs it without borrowing the engine.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Grammar(usize);

/// Which scope wears which pen, in the form the engine matches against.
pub struct Palette {
    theme: Theme,
}

/// How far through a file the engine has read, and what it was in the middle
/// of.
///
/// Two states, not one: `ParseState` knows which grammar contexts are open —
/// whether this line is inside a block comment — and `HighlightState` knows
/// which theme rules those contexts resolved to. Both must be carried from one
/// line to the next, so a highlighter cannot be asked about line 500 alone.
pub struct SyntectState {
    parse: ParseState,
    highlight: HighlightState,
    /// The line with its newline restored, reused so that engine_state a file does
    /// not allocate once per line.
    ///
    /// The grammars are the newline-terminated variants, because a rule that
    /// ends at `$` needs something to match against; without it a line comment
    /// never closes.
    buffer: String,
}

impl Engine {
    pub fn new() -> Self {
        Self {
            syntaxes: two_face::syntax::extra_newlines(),
        }
    }

    /// Which grammar reads this file, if any does.
    ///
    /// Name, then extension, then shebang — most certain first. A file nothing
    /// claims gets `None` and is shown as plain text, which is the S11
    /// criterion for an unrecognised type: no colour, no failure.
    pub fn find(&self, clues: Clues<'_>) -> Option<Grammar> {
        let by_name = clues
            .well_known()
            .and_then(|name| self.syntaxes.find_syntax_by_name(name));
        let by_extension = || {
            clues
                .extension()
                .and_then(|ext| self.syntaxes.find_syntax_by_extension(ext))
        };
        // The whole file name too: as far as the engine's own table is
        // concerned, `.gitignore` and `Makefile` are extensions.
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

    /// What the engine calls this grammar, for tests and for a status line.
    pub fn name(&self, grammar: Grammar) -> &str {
        &self.syntaxes.syntaxes()[grammar.0].name
    }

    /// Begins engine_state a file from its first line.
    pub fn start(&self, grammar: Grammar, palette: &Palette) -> SyntectState {
        let syntax = &self.syntaxes.syntaxes()[grammar.0];
        let highlighter = Highlighter::new(&palette.theme);
        SyntectState {
            parse: ParseState::new(syntax),
            highlight: HighlightState::new(&highlighter, ScopeStack::new()),
            buffer: String::new(),
        }
    }

    /// Reads the given lines in order, appending the spans for each.
    ///
    /// A batch rather than one line at a time for one measured reason: the
    /// engine's matcher is built from the theme, and building it costs a pass
    /// over every rule. Per line that was two thirds of the total — 15 000
    /// lines a second became 45 000 by moving one constructor out of the loop.
    /// The caller already has the whole slice, so there is nothing to give up.
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
    /// Must be called in order from the first line: the answer for this line
    /// depends on every line before it. A line too long to be worth colouring
    /// is still parsed — its state is carried forward — but reported as
    /// having no spans, so a minified bundle cannot corrupt the lines after
    /// it. That is `bat`'s answer; `delta` truncates the text and loses the
    /// state.
    fn read_line(&self, engine_state: &mut SyntectState, matcher: &Highlighter<'_>, line: &str) -> Vec<Span> {
        engine_state.buffer.clear();
        engine_state.buffer.push_str(line);
        engine_state.buffer.push('\n');

        let Ok(ops) = engine_state.parse.parse_line(&engine_state.buffer, &self.syntaxes) else {
            // A grammar that failed on one line has not failed on the file.
            return Vec::new();
        };
        if !limits::worth_colouring(line) {
            // Keep the parse state, drop the colour.
            return Vec::new();
        }

        let spans =
            RangedHighlightIterator::new(&mut engine_state.highlight, &ops, &engine_state.buffer, matcher)
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
    /// Builds the engine's theme from the caller's rules.
    ///
    /// A rule whose selector the engine cannot parse is dropped rather than
    /// failing the whole theme: one bad selector should cost one colour, not
    /// every colour. [`rules`](Self::rules) reports how many were accepted so
    /// a test can catch one we wrote wrongly.
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
                // A transparent default, so that a scope no rule claimed can
                // be told apart from one a rule painted. Every rule we build
                // is opaque, so alpha alone answers it — see `convert`.
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
        // Never a background: a diff owns that, and a syntax background would
        // hide which lines changed. See the crate README.
        background: None,
        font_style: Some(font),
    }
}

/// A [`Pen`] hidden in the only field the engine will carry for us.
///
/// The engine resolves a *colour* per scope, so a pen number rides in one. It
/// is never shown to a terminal: [`decode`] takes it back out before a span
/// leaves this file. `bat` smuggles ANSI indices through the same field for
/// the same reason — there is nowhere else to put them.
///
/// Alpha is the tell. Every pen we encode is opaque; [`UNCLAIMED`] is not.
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

/// What a scope resolves to when no rule claimed it.
///
/// Transparent, which no pen of ours ever is, so the two cannot be confused.
/// Comparing against a *value* instead would mean a theme could not use that
/// pen, and getting it wrong is invisible: every ordinary character would be
/// repainted in a shade `ui` never chose, which is what the first draft of
/// this did.
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
