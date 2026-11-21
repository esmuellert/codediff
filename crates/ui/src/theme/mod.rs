//! Colours.

pub mod basic;
pub mod catppuccin;
pub mod change;
pub mod code;
mod colour;
pub mod icon;
pub mod probe;
pub mod tree;

use ratatui::style::Style;

pub use catppuccin::Flavour;
pub use change::Change;
pub use code::Code;
pub use colour::{Rgb, blend};
pub use icon::Icon;
pub use probe::{TerminalProbe, probe};
pub use tree::Tree;

/// Every style the interface draws with.
#[derive(Debug, Clone, Copy)]
pub struct Theme {
    /// Stable name used by diagnostics and theme lookup.
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
    pub deleted_text: Style,
    pub inserted_text: Style,
    /// A block marked as moved.
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
    /// Warning style over the status style.
    pub warning: Style,

    /// Mouse text selection highlight.
    pub selection: Style,

    /// The colour of each kind of code. See [`code`].
    pub code: Code,

    /// The colour of each part of a tree drawn in terminal lines. See [`tree`].
    pub tree: Tree,

    /// The colour of each way a file can have changed. See [`change`].
    pub change: Change,

    /// The border of a pane.
    pub border: Style,
    /// The border of the focused pane.
    pub border_focused: Style,
}

impl Theme {
    /// Catppuccin Mocha — the default on a terminal that can show it.
    pub const DARK: Self = catppuccin::theme(Flavour::Mocha);
    /// Catppuccin Latte.
    pub const LIGHT: Self = catppuccin::theme(Flavour::Latte);

    /// Every built-in theme.
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

    /// Looks up a built-in theme by name.
    pub fn named(name: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|theme| theme.name == name)
    }

    /// Picks a theme from environment variables alone.
    pub fn detect(environment: impl Fn(&str) -> Option<String>) -> Self {
        Self::resolve(TerminalProbe::default(), environment)
    }

    /// Picks a theme from a terminal probe and environment variables.
    pub fn resolve(probed: TerminalProbe, environment: impl Fn(&str) -> Option<String>) -> Self {
        let light = probed
            .background
            .map(|bg| !bg.is_dark())
            .unwrap_or_else(|| prefers_light(&environment));
        let truecolor = supports_truecolor(probed, &environment);

        match (truecolor, light) {
            (true, false) => Self::DARK,
            (true, true) => Self::LIGHT,
            (false, false) => basic::DARK,
            (false, true) => basic::LIGHT,
        }
    }

    /// Detects a theme from the process environment without touching the TTY.
    pub fn from_environment() -> Self {
        Self::detect(|key| std::env::var(key).ok())
    }

    /// Queries the attached terminal and falls back to the environment.
    pub fn from_terminal() -> Self {
        Self::resolve(probe::probe(), |key| std::env::var(key).ok())
    }

    /// Every style, for tests that need to check all of them at once.
    pub fn styles(&self) -> [Style; 15] {
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
            self.selection,
        ]
    }
}

fn supports_truecolor(
    probed: TerminalProbe,
    environment: &impl Fn(&str) -> Option<String>,
) -> bool {
    if environment("NO_COLOR").is_some_and(|value| !value.is_empty()) {
        return false;
    }
    if let Some(term) = environment("TERM")
        && (term.eq_ignore_ascii_case("dumb") || term.eq_ignore_ascii_case("linux"))
    {
        return false;
    }
    if let Some(colorterm) = environment("COLORTERM") {
        if colorterm.eq_ignore_ascii_case("truecolor") || colorterm.eq_ignore_ascii_case("24bit") {
            return true;
        }
        if !colorterm.is_empty() {
            return false;
        }
    }
    if probed.truecolor == Some(true) {
        return true;
    }
    let apple_terminal = environment("TERM_PROGRAM")
        .is_some_and(|program| program.eq_ignore_ascii_case("Apple_Terminal"));
    if apple_terminal {
        return false;
    }
    if probed.background.is_some() {
        return true;
    }
    env_truecolor(environment)
}

fn env_truecolor(environment: &impl Fn(&str) -> Option<String>) -> bool {
    if let Some(term) = environment("TERM") {
        let lower = term.to_ascii_lowercase();
        if lower.ends_with("-direct")
            || lower.ends_with("-truecolor")
            || matches!(
                lower.as_str(),
                "xterm-kitty"
                    | "xterm-ghostty"
                    | "ghostty"
                    | "wezterm"
                    | "alacritty"
                    | "foot"
                    | "foot-extra"
                    | "rio"
                    | "contour"
            )
        {
            return true;
        }
    }
    if let Some(program) = environment("TERM_PROGRAM") {
        let lower = program.to_ascii_lowercase();
        if matches!(
            lower.as_str(),
            "iterm.app"
                | "wezterm"
                | "ghostty"
                | "vscode"
                | "hyper"
                | "warpterminal"
                | "zed"
                | "tabby"
                | "rio"
                | "alacritty"
        ) {
            return true;
        }
    }
    environment("WT_SESSION").is_some_and(|value| !value.is_empty())
        || environment("KONSOLE_VERSION").is_some_and(|value| !value.is_empty())
        || environment("VTE_VERSION")
            .and_then(|value| value.parse::<u32>().ok())
            .is_some_and(|version| version >= 3600)
}

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
    fn osc11_probe_selects_24bit_mocha_or_latte_without_env_vars() {
        let env = [("TERM", "xterm-256color")];
        let dark = TerminalProbe {
            background: Some(Rgb(0x1e, 0x1e, 0x2e)),
            truecolor: None,
        };
        assert_eq!(
            Theme::resolve(dark, environment(&env)).name,
            "catppuccin-mocha"
        );

        let light = TerminalProbe {
            background: Some(Rgb(0xef, 0xf1, 0xf5)),
            truecolor: None,
        };
        assert_eq!(
            Theme::resolve(light, environment(&env)).name,
            "catppuccin-latte"
        );
    }

    #[test]
    fn apple_terminal_uses_probed_lightness_without_claiming_truecolor() {
        let env = [
            ("TERM", "xterm-256color"),
            ("TERM_PROGRAM", "Apple_Terminal"),
        ];
        let light = TerminalProbe {
            background: Some(Rgb(0xff, 0xff, 0xff)),
            truecolor: Some(false),
        };
        assert_eq!(Theme::resolve(light, environment(&env)).name, "basic-light");

        let dark = TerminalProbe {
            background: Some(Rgb(0x10, 0x10, 0x10)),
            truecolor: None,
        };
        assert_eq!(Theme::resolve(dark, environment(&env)).name, "basic-dark");
    }

    #[test]
    fn known_truecolor_terminals_are_recognised_when_osc11_is_unavailable() {
        let unprobed = TerminalProbe::default();
        for pair in [
            ("TERM", "xterm-kitty"),
            ("TERM", "xterm-ghostty"),
            ("TERM", "wezterm"),
            ("TERM", "tmux-direct"),
            ("TERM_PROGRAM", "iTerm.app"),
            ("TERM_PROGRAM", "WezTerm"),
            ("TERM_PROGRAM", "ghostty"),
            ("WT_SESSION", "1"),
        ] {
            assert_eq!(
                Theme::resolve(unprobed, environment(&[pair])).name,
                "catppuccin-mocha",
                "{pair:?}"
            );
        }
    }

    #[test]
    fn no_color_and_dumb_terminals_stay_basic_even_when_probed() {
        let probed = TerminalProbe {
            background: Some(Rgb(0xff, 0xff, 0xff)),
            truecolor: Some(true),
        };
        assert_eq!(
            Theme::resolve(probed, environment(&[("NO_COLOR", "1")])).name,
            "basic-light"
        );
        assert_eq!(
            Theme::resolve(probed, environment(&[("TERM", "dumb")])).name,
            "basic-light"
        );
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
        let theme = Theme::DARK;
        let colour = theme.normal.patch(theme.inserted);
        assert_eq!(
            colour.fg, theme.normal.fg,
            "the role did not set a foreground"
        );
        assert_eq!(colour.bg, theme.inserted.bg);
    }
}
