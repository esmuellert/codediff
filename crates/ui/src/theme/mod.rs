//! Colours.
//!
//! ---
//!
//! A [`Theme`] is a table of `Style`s, one per role, and nothing else. It has
//! no idea what a hunk is, and the renderer has no idea what Catppuccin is;
//! the two meet at the field names below.
//!
//! Each field is a **complete** style rather than a colour, so a theme can say
//! bold or reversed where a colour would not carry — which is what lets
//! [`basic`] work on a terminal that has no 24-bit colour to give.
//!
//! Styles compose by [`Style::patch`], which overrides only the fields that
//! are set. A row is `normal` patched with its role; its gutter is that
//! patched with `line_number`, which sets a foreground and so keeps the row's
//! background. Priority is therefore the order of the patches, written out at
//! each use rather than encoded in a table nobody can read.

pub mod basic;
pub mod catppuccin;
mod colour;

use ratatui::style::Style;

pub use catppuccin::Flavour;
pub use colour::{Rgb, blend};

/// Every style the interface draws with.
#[derive(Debug, Clone, Copy)]
pub struct Theme {
    /// What `--theme` calls this one.
    pub name: &'static str,
    /// Whether it expects a dark terminal. Used only to pick a default.
    pub dark: bool,

    /// Ordinary text on the ordinary background.
    pub normal: Style,

    /// A line only on the original side, or differing there.
    pub deleted: Style,
    /// A line only on the modified side, or differing there.
    pub inserted: Style,
    /// The characters within such a line that actually differ.
    ///
    /// Drawn over the line's own style, so it must be visibly stronger.
    pub deleted_text: Style,
    pub inserted_text: Style,
    /// A block the engine judged to have moved rather than been rewritten.
    pub moved: Style,

    /// The `╱` hatching where one side has no line at all.
    pub filler: Style,
    pub line_number: Style,
    pub line_number_current: Style,
    pub cursor_line: Style,
    pub divider: Style,

    pub status: Style,
    /// Patched over `status` for the file name.
    pub status_path: Style,
    /// Patched over `status` for something the reader must not miss.
    pub warning: Style,
}

impl Theme {
    /// Catppuccin Mocha — the default on a terminal that can show it.
    pub const DARK: Self = catppuccin::theme(Flavour::Mocha);
    /// Catppuccin Latte.
    pub const LIGHT: Self = catppuccin::theme(Flavour::Latte);

    /// Every theme, in the order `--theme` lists them.
    pub const ALL: [Self; 6] = [
        catppuccin::theme(Flavour::Mocha),
        catppuccin::theme(Flavour::Macchiato),
        catppuccin::theme(Flavour::Frappe),
        catppuccin::theme(Flavour::Latte),
        basic::DARK,
        basic::LIGHT,
    ];

    pub const NAMES: [&'static str; 6] = [
        "catppuccin-mocha",
        "catppuccin-macchiato",
        "catppuccin-frappe",
        "catppuccin-latte",
        "basic-dark",
        "basic-light",
    ];

    /// Looks a theme up by the name `--theme` uses.
    pub fn named(name: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|theme| theme.name == name)
    }

    /// What to use when the reader has not said.
    ///
    /// Catppuccin unless the terminal cannot show it. Its diff backgrounds are
    /// eighteen percent of an accent over the base — a few points of
    /// lightness — and a terminal without 24-bit colour rounds them straight
    /// back into the background, leaving a diff with no visible diff in it.
    /// Where that is a risk, [`basic`] is used instead: it is less pretty and
    /// it always works.
    ///
    /// Detection is by environment variable because there is no way to ask.
    /// `COLORTERM` is what every terminal that supports 24-bit colour sets,
    /// and the check is deliberately one-way — an unset variable means "not
    /// sure", and being unsure is a reason to pick the theme that cannot fail.
    pub fn detect(environment: impl Fn(&str) -> Option<String>) -> Self {
        let truecolor = environment("COLORTERM")
            .is_some_and(|value| value.eq_ignore_ascii_case("truecolor") || value == "24bit");

        match (truecolor, prefers_light(&environment)) {
            (true, false) => Self::DARK,
            (true, true) => Self::LIGHT,
            (false, false) => basic::DARK,
            (false, true) => basic::LIGHT,
        }
    }

    /// The same, reading the real environment.
    pub fn from_environment() -> Self {
        Self::detect(|key| std::env::var(key).ok())
    }

    /// Every style, for tests that need to check all of them at once.
    pub fn styles(&self) -> [Style; 14] {
        [
            self.normal,
            self.deleted,
            self.inserted,
            self.deleted_text,
            self.inserted_text,
            self.moved,
            self.filler,
            self.line_number,
            self.line_number_current,
            self.cursor_line,
            self.divider,
            self.status,
            self.status_path,
            self.warning,
        ]
    }
}

/// Whether the terminal is likely to have a light background.
///
/// There is a real way to ask — an OSC 11 query — but it needs a round trip
/// the terminal may never answer, and a reviewer waiting on a timeout before
/// the first frame is worse than a wrong guess they can override. So: only
/// what is already known for free.
fn prefers_light(environment: &impl Fn(&str) -> Option<String>) -> bool {
    // The convention several terminals and `vim` itself use.
    environment("COLORFGBG").is_some_and(|value| {
        // `foreground;background`, as palette indices. 7 and 15 are white.
        value
            .rsplit(';')
            .next()
            .and_then(|bg| bg.parse::<u8>().ok())
            .is_some_and(|bg| (7..=15).contains(&bg))
    })
}

impl Default for Theme {
    fn default() -> Self {
        Self::DARK
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn environment(pairs: &[(&str, &str)]) -> impl Fn(&str) -> Option<String> + use<> {
        let pairs: Vec<(String, String)> = pairs
            .iter()
            .map(|(k, v)| ((*k).to_owned(), (*v).to_owned()))
            .collect();
        move |key| {
            pairs
                .iter()
                .find(|(k, _)| k == key)
                .map(|(_, v)| v.to_owned())
        }
    }

    #[test]
    fn every_name_resolves_and_every_theme_is_named() {
        for name in Theme::NAMES {
            assert_eq!(Theme::named(name).expect(name).name, name);
        }
        for theme in Theme::ALL {
            assert!(Theme::NAMES.contains(&theme.name), "{}", theme.name);
        }
    }

    #[test]
    fn an_unknown_name_is_refused_rather_than_silently_defaulted() {
        assert!(Theme::named("dracula").is_none());
        assert!(Theme::named("").is_none());
        assert!(Theme::named("CATPPUCCIN-MOCHA").is_none());
    }

    #[test]
    fn a_truecolor_terminal_gets_catppuccin() {
        let theme = Theme::detect(environment(&[("COLORTERM", "truecolor")]));
        assert_eq!(theme.name, "catppuccin-mocha");
        let theme = Theme::detect(environment(&[("COLORTERM", "24bit")]));
        assert_eq!(theme.name, "catppuccin-mocha");
    }

    #[test]
    fn a_terminal_that_says_nothing_gets_the_theme_that_cannot_fail() {
        // Catppuccin's diff backgrounds are a few points of lightness over the
        // base; quantised to 256 colours they vanish entirely.
        assert_eq!(Theme::detect(environment(&[])).name, "basic-dark");
        assert_eq!(
            Theme::detect(environment(&[("COLORTERM", "")])).name,
            "basic-dark"
        );
        assert_eq!(
            Theme::detect(environment(&[("TERM", "xterm-256color")])).name,
            "basic-dark"
        );
    }

    #[test]
    fn a_light_terminal_gets_a_light_theme_either_way() {
        let light = [("COLORFGBG", "0;15")];
        assert_eq!(
            Theme::detect(environment(&[light[0], ("COLORTERM", "truecolor")])).name,
            "catppuccin-latte"
        );
        assert_eq!(Theme::detect(environment(&light)).name, "basic-light");
    }

    #[test]
    fn a_dark_colorfgbg_is_not_mistaken_for_a_light_one() {
        for background in ["15;0", "7;0", "0", "default;default", ""] {
            let theme = Theme::detect(environment(&[
                ("COLORFGBG", background),
                ("COLORTERM", "truecolor"),
            ]));
            assert_eq!(theme.name, "catppuccin-mocha", "COLORFGBG={background:?}");
        }
    }

    #[test]
    fn every_theme_distinguishes_the_roles_that_must_differ() {
        for theme in Theme::ALL {
            let name = theme.name;
            assert_ne!(theme.inserted.bg, theme.deleted.bg, "{name}");
            assert_ne!(theme.inserted.bg, theme.normal.bg, "{name}");
            assert_ne!(theme.deleted.bg, theme.normal.bg, "{name}");
            assert_ne!(theme.inserted.bg, theme.inserted_text.bg, "{name}");
            assert_ne!(theme.deleted.bg, theme.deleted_text.bg, "{name}");
            assert_ne!(theme.cursor_line.bg, theme.normal.bg, "{name}");
            assert_ne!(theme.status.bg, theme.normal.bg, "{name}");
        }
    }

    #[test]
    fn the_dark_flag_agrees_with_the_name() {
        for theme in Theme::ALL {
            assert_eq!(
                theme.dark,
                !theme.name.ends_with("light") && !theme.name.ends_with("latte"),
                "{}",
                theme.name
            );
        }
    }

    #[test]
    fn patching_a_role_over_normal_keeps_what_the_role_does_not_set() {
        // How every row is built: the role supplies a background and inherits
        // the foreground, so text stays readable without each role having to
        // repeat it.
        let theme = Theme::DARK;
        let row = theme.normal.patch(theme.inserted);
        assert_eq!(row.fg, theme.normal.fg, "the role did not set a foreground");
        assert_eq!(row.bg, theme.inserted.bg);
    }
}
