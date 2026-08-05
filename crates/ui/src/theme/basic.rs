//! A theme that works on a terminal we know nothing about.
//!
//! Catppuccin names exact colours, which requires the terminal to accept
//! 24-bit escapes. Not all do: `tmux` without `-2`, an old `TERM`, a
//! conservative SSH session. Where they are unavailable the terminal quantises
//! them, and Catppuccin's diff backgrounds — eighteen percent of an accent —
//! are exactly the colours that collapse into the background when it does.
//!
//! So this theme names nothing exactly. Its background is the terminal's own,
//! and its diff colours come from the 256-colour cube, which every terminal
//! from the last twenty years has. It looks like part of whatever colour
//! scheme the reader already runs, which is the point.

use ratatui::style::{Color, Modifier, Style};

use crate::theme::Theme;
use crate::theme::code::Code;

/// Indices into the 6×6×6 colour cube that starts at 16.
///
/// `16 + 36r + 6g + 6b`, each component 0..=5. Chosen dark enough to sit under
/// text rather than compete with it — the same job Catppuccin's 18% does, done
/// with the resolution available.
mod cube {
    /// `r0 g1 b0` — the faintest green in the cube.
    pub const DARK_GREEN: u8 = 22;
    /// `r1 g0 b0`.
    pub const DARK_RED: u8 = 52;
    /// `r0 g2 b0`.
    pub const GREEN: u8 = 28;
    /// `r2 g0 b0`.
    pub const RED: u8 = 88;
    /// `r0 g0 b1`.
    pub const DARK_BLUE: u8 = 17;
    /// `r0 g0 b2`.
    pub const GREY: u8 = 236;

    /// `r4 g5 b4` — the faintest tint that still reads as green on white.
    pub const PALE_GREEN: u8 = 194;
    pub const PALE_RED: u8 = 224;
    pub const LIGHT_GREEN: u8 = 157;
    pub const LIGHT_RED: u8 = 217;
    pub const PALE_BLUE: u8 = 189;
    pub const LIGHT_GREY: u8 = 254;
}

const fn over(index: u8) -> Style {
    Style::new().bg(Color::Indexed(index))
}

const fn ink(colour: Color) -> Style {
    Style::new().fg(colour)
}

/// Syntax colours from the sixteen every terminal has.
///
/// The same groups Catppuccin parts, resolved onto a palette a quarter the
/// size, so several of them necessarily land together — `constant` and
/// `library` share a colour here because Catppuccin gives both peach, and
/// `character` and `operator` share one because teal and sky are neighbours.
/// The groups stay apart in the table so a richer theme can part them; only
/// this rendering of them collapses.
///
/// The bright half on a dark background and the plain half on a light one:
/// bright yellow on white is unreadable, and plain yellow on black is dim.
const DARK_CODE: Code = Code {
    comment: Color::DarkGray,
    string: Color::LightGreen,
    character: Color::Cyan,
    escape: Color::Magenta,
    regexp: Color::Magenta,
    constant: Color::Yellow,
    keyword: Color::LightMagenta,
    operator: Color::Cyan,
    preprocessor: Color::Magenta,
    kind: Color::LightYellow,
    function: Color::LightBlue,
    library: Color::Yellow,
    // The terminal's own foreground: an ordinary name should look ordinary,
    // which is the same argument as `normal` above.
    variable: Color::Reset,
    builtin: Color::LightRed,
    parameter: Color::Red,
    property: Color::LightCyan,
    namespace: Color::LightYellow,
    label: Color::Blue,
    punctuation: Color::DarkGray,
    tag: Color::LightBlue,
    attribute: Color::LightYellow,
    invalid: Color::LightRed,
    heading: Color::LightBlue,
    link: Color::Blue,
    reference: Color::LightCyan,
    raw: Color::LightGreen,
    list: Color::Cyan,
    quote: Color::Magenta,
    emphasis: Color::LightRed,
    inserted: Color::LightGreen,
    deleted: Color::LightRed,
};

/// The same groups for a light terminal. See [`DARK_CODE`].
const LIGHT_CODE: Code = Code {
    comment: Color::DarkGray,
    string: Color::Green,
    character: Color::Cyan,
    escape: Color::Magenta,
    regexp: Color::Magenta,
    constant: Color::Red,
    keyword: Color::Magenta,
    operator: Color::Cyan,
    preprocessor: Color::Magenta,
    kind: Color::Yellow,
    function: Color::Blue,
    library: Color::Red,
    variable: Color::Reset,
    builtin: Color::Red,
    parameter: Color::Magenta,
    property: Color::Cyan,
    namespace: Color::Yellow,
    label: Color::Blue,
    punctuation: Color::DarkGray,
    tag: Color::Blue,
    attribute: Color::Yellow,
    invalid: Color::Red,
    heading: Color::Blue,
    link: Color::Blue,
    reference: Color::Magenta,
    raw: Color::Green,
    list: Color::Cyan,
    quote: Color::Magenta,
    emphasis: Color::Red,
    inserted: Color::Green,
    deleted: Color::Red,
};

/// For a terminal with a dark background.
pub const DARK: Theme = Theme {
    name: "basic-dark",
    dark: true,

    // `Reset` means "whatever the terminal already uses", so an unchanged line
    // is indistinguishable from the surrounding shell — which is what makes
    // this theme fit in anywhere.
    normal: Style::new().fg(Color::Reset).bg(Color::Reset),

    deleted: over(cube::DARK_RED),
    inserted: over(cube::DARK_GREEN),
    deleted_text: over(cube::RED),
    inserted_text: over(cube::GREEN),
    moved: over(cube::DARK_BLUE),

    filler: Style::new().fg(Color::DarkGray).bg(Color::Reset),
    line_number: ink(Color::DarkGray),
    line_number_current: ink(Color::White),
    cursor_line: over(cube::GREY),
    divider: Style::new().fg(Color::DarkGray).bg(Color::Reset),

    status: Style::new().fg(Color::Black).bg(Color::Gray),
    status_path: Style::new().add_modifier(Modifier::BOLD),
    warning: Style::new()
        .fg(Color::Red)
        .add_modifier(Modifier::BOLD)
        .add_modifier(Modifier::REVERSED),

    code: DARK_CODE,
};

/// For a terminal with a light background.
pub const LIGHT: Theme = Theme {
    name: "basic-light",
    dark: false,

    normal: Style::new().fg(Color::Reset).bg(Color::Reset),

    deleted: over(cube::PALE_RED),
    inserted: over(cube::PALE_GREEN),
    deleted_text: over(cube::LIGHT_RED),
    inserted_text: over(cube::LIGHT_GREEN),
    moved: over(cube::PALE_BLUE),

    filler: Style::new().fg(Color::Gray).bg(Color::Reset),
    line_number: ink(Color::Gray),
    line_number_current: ink(Color::Black),
    cursor_line: over(cube::LIGHT_GREY),
    divider: Style::new().fg(Color::Gray).bg(Color::Reset),

    status: Style::new().fg(Color::White).bg(Color::DarkGray),
    status_path: Style::new().add_modifier(Modifier::BOLD),
    warning: Style::new()
        .fg(Color::Red)
        .add_modifier(Modifier::BOLD)
        .add_modifier(Modifier::REVERSED),

    code: LIGHT_CODE,
};

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn nothing_here_names_a_24_bit_colour() {
        // The entire reason this theme exists. One `Color::Rgb` and it would
        // fail on exactly the terminals it is meant for.
        for theme in [DARK, LIGHT] {
            for style in theme.styles() {
                for colour in [style.fg, style.bg] {
                    assert!(
                        !matches!(colour, Some(Color::Rgb(..))),
                        "{}: {colour:?}",
                        theme.name
                    );
                }
            }
            // Syntax colours too: they are the largest table here, and the
            // one most easily filled in by copying a 24-bit theme.
            for token in syntax::Group::ALL {
                assert!(
                    !matches!(theme.code.colour(token), Color::Rgb(..)),
                    "{}: {}",
                    theme.name,
                    token.name()
                );
            }
        }
    }

    #[test]
    fn an_unchanged_line_inherits_the_terminals_own_colours() {
        for theme in [DARK, LIGHT] {
            assert_eq!(theme.normal.bg, Some(Color::Reset), "{}", theme.name);
            assert_eq!(theme.normal.fg, Some(Color::Reset), "{}", theme.name);
        }
    }

    #[test]
    fn changed_characters_are_a_different_colour_from_their_line() {
        for theme in [DARK, LIGHT] {
            assert_ne!(theme.inserted.bg, theme.inserted_text.bg, "{}", theme.name);
            assert_ne!(theme.deleted.bg, theme.deleted_text.bg, "{}", theme.name);
        }
    }

    #[test]
    fn the_two_variants_do_not_share_a_single_colour() {
        // If they did, one of them was not thought about.
        assert_ne!(DARK.inserted.bg, LIGHT.inserted.bg);
        assert_ne!(DARK.cursor_line.bg, LIGHT.cursor_line.bg);
        assert_ne!(DARK.status.bg, LIGHT.status.bg);
    }
}
