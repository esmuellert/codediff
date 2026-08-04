//! Catppuccin.
//!
//! Four flavours, reproduced by their *arithmetic*. The palette below is the
//! published one; every colour the interface uses is derived from it by
//! [`blend`] at the opacities `catppuccin/nvim` uses for its own highlight
//! groups. Nothing here is a hex value copied out of a screenshot, so a
//! flavour is 26 numbers and a shared derivation rather than 26 numbers and
//! fourteen more that have to be kept in step with them.
//!
//! Source: <https://github.com/catppuccin/nvim> `lua/catppuccin/palettes/`.

use ratatui::style::{Modifier, Style};

use crate::theme::Theme;
use crate::theme::colour::{Rgb, blend};

/// The four flavours, dark to light.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Flavour {
    #[default]
    Mocha,
    Macchiato,
    Frappe,
    Latte,
}

impl Flavour {
    pub const ALL: [Flavour; 4] = [
        Flavour::Mocha,
        Flavour::Macchiato,
        Flavour::Frappe,
        Flavour::Latte,
    ];

    pub const fn palette(self) -> Palette {
        match self {
            Flavour::Mocha => MOCHA,
            Flavour::Macchiato => MACCHIATO,
            Flavour::Frappe => FRAPPE,
            Flavour::Latte => LATTE,
        }
    }

    pub const fn name(self) -> &'static str {
        match self {
            Flavour::Mocha => "catppuccin-mocha",
            Flavour::Macchiato => "catppuccin-macchiato",
            Flavour::Frappe => "catppuccin-frappe",
            Flavour::Latte => "catppuccin-latte",
        }
    }
}

/// The 26 named colours of one flavour.
#[derive(Debug, Clone, Copy)]
pub struct Palette {
    pub rosewater: Rgb,
    pub flamingo: Rgb,
    pub pink: Rgb,
    pub mauve: Rgb,
    pub red: Rgb,
    pub maroon: Rgb,
    pub peach: Rgb,
    pub yellow: Rgb,
    pub green: Rgb,
    pub teal: Rgb,
    pub sky: Rgb,
    pub sapphire: Rgb,
    pub blue: Rgb,
    pub lavender: Rgb,
    pub text: Rgb,
    pub subtext1: Rgb,
    pub subtext0: Rgb,
    pub overlay2: Rgb,
    pub overlay1: Rgb,
    pub overlay0: Rgb,
    pub surface2: Rgb,
    pub surface1: Rgb,
    pub surface0: Rgb,
    pub base: Rgb,
    pub mantle: Rgb,
    pub crust: Rgb,
}

pub const LATTE: Palette = Palette {
    rosewater: Rgb(0xdc, 0x8a, 0x78),
    flamingo: Rgb(0xdd, 0x78, 0x78),
    pink: Rgb(0xea, 0x76, 0xcb),
    mauve: Rgb(0x88, 0x39, 0xef),
    red: Rgb(0xd2, 0x0f, 0x39),
    maroon: Rgb(0xe6, 0x45, 0x53),
    peach: Rgb(0xfe, 0x64, 0x0b),
    yellow: Rgb(0xdf, 0x8e, 0x1d),
    green: Rgb(0x40, 0xa0, 0x2b),
    teal: Rgb(0x17, 0x92, 0x99),
    sky: Rgb(0x04, 0xa5, 0xe5),
    sapphire: Rgb(0x20, 0x9f, 0xb5),
    blue: Rgb(0x1e, 0x66, 0xf5),
    lavender: Rgb(0x72, 0x87, 0xfd),
    text: Rgb(0x4c, 0x4f, 0x69),
    subtext1: Rgb(0x5c, 0x5f, 0x77),
    subtext0: Rgb(0x6c, 0x6f, 0x85),
    overlay2: Rgb(0x7c, 0x7f, 0x93),
    overlay1: Rgb(0x8c, 0x8f, 0xa1),
    overlay0: Rgb(0x9c, 0xa0, 0xb0),
    surface2: Rgb(0xac, 0xb0, 0xbe),
    surface1: Rgb(0xbc, 0xc0, 0xcc),
    surface0: Rgb(0xcc, 0xd0, 0xda),
    base: Rgb(0xef, 0xf1, 0xf5),
    mantle: Rgb(0xe6, 0xe9, 0xef),
    crust: Rgb(0xdc, 0xe0, 0xe8),
};

pub const FRAPPE: Palette = Palette {
    rosewater: Rgb(0xf2, 0xd5, 0xcf),
    flamingo: Rgb(0xee, 0xbe, 0xbe),
    pink: Rgb(0xf4, 0xb8, 0xe4),
    mauve: Rgb(0xca, 0x9e, 0xe6),
    red: Rgb(0xe7, 0x82, 0x84),
    maroon: Rgb(0xea, 0x99, 0x9c),
    peach: Rgb(0xef, 0x9f, 0x76),
    yellow: Rgb(0xe5, 0xc8, 0x90),
    green: Rgb(0xa6, 0xd1, 0x89),
    teal: Rgb(0x81, 0xc8, 0xbe),
    sky: Rgb(0x99, 0xd1, 0xdb),
    sapphire: Rgb(0x85, 0xc1, 0xdc),
    blue: Rgb(0x8c, 0xaa, 0xee),
    lavender: Rgb(0xba, 0xbb, 0xf1),
    text: Rgb(0xc6, 0xd0, 0xf5),
    subtext1: Rgb(0xb5, 0xbf, 0xe2),
    subtext0: Rgb(0xa5, 0xad, 0xce),
    overlay2: Rgb(0x94, 0x9c, 0xbb),
    overlay1: Rgb(0x83, 0x8b, 0xa7),
    overlay0: Rgb(0x73, 0x79, 0x94),
    surface2: Rgb(0x62, 0x68, 0x80),
    surface1: Rgb(0x51, 0x57, 0x6d),
    surface0: Rgb(0x41, 0x45, 0x59),
    base: Rgb(0x30, 0x34, 0x46),
    mantle: Rgb(0x29, 0x2c, 0x3c),
    crust: Rgb(0x23, 0x26, 0x34),
};

pub const MACCHIATO: Palette = Palette {
    rosewater: Rgb(0xf4, 0xdb, 0xd6),
    flamingo: Rgb(0xf0, 0xc6, 0xc6),
    pink: Rgb(0xf5, 0xbd, 0xe6),
    mauve: Rgb(0xc6, 0xa0, 0xf6),
    red: Rgb(0xed, 0x87, 0x96),
    maroon: Rgb(0xee, 0x99, 0xa0),
    peach: Rgb(0xf5, 0xa9, 0x7f),
    yellow: Rgb(0xee, 0xd4, 0x9f),
    green: Rgb(0xa6, 0xda, 0x95),
    teal: Rgb(0x8b, 0xd5, 0xca),
    sky: Rgb(0x91, 0xd7, 0xe3),
    sapphire: Rgb(0x7d, 0xc4, 0xe4),
    blue: Rgb(0x8a, 0xad, 0xf4),
    lavender: Rgb(0xb7, 0xbd, 0xf8),
    text: Rgb(0xca, 0xd3, 0xf5),
    subtext1: Rgb(0xb8, 0xc0, 0xe0),
    subtext0: Rgb(0xa5, 0xad, 0xcb),
    overlay2: Rgb(0x93, 0x9a, 0xb7),
    overlay1: Rgb(0x80, 0x87, 0xa2),
    overlay0: Rgb(0x6e, 0x73, 0x8d),
    surface2: Rgb(0x5b, 0x60, 0x78),
    surface1: Rgb(0x49, 0x4d, 0x64),
    surface0: Rgb(0x36, 0x3a, 0x4f),
    base: Rgb(0x24, 0x27, 0x3a),
    mantle: Rgb(0x1e, 0x20, 0x30),
    crust: Rgb(0x18, 0x19, 0x26),
};

pub const MOCHA: Palette = Palette {
    rosewater: Rgb(0xf5, 0xe0, 0xdc),
    flamingo: Rgb(0xf2, 0xcd, 0xcd),
    pink: Rgb(0xf5, 0xc2, 0xe7),
    mauve: Rgb(0xcb, 0xa6, 0xf7),
    red: Rgb(0xf3, 0x8b, 0xa8),
    maroon: Rgb(0xeb, 0xa0, 0xac),
    peach: Rgb(0xfa, 0xb3, 0x87),
    yellow: Rgb(0xf9, 0xe2, 0xaf),
    green: Rgb(0xa6, 0xe3, 0xa1),
    teal: Rgb(0x94, 0xe2, 0xd5),
    sky: Rgb(0x89, 0xdc, 0xeb),
    sapphire: Rgb(0x74, 0xc7, 0xec),
    blue: Rgb(0x89, 0xb4, 0xfa),
    lavender: Rgb(0xb4, 0xbe, 0xfe),
    text: Rgb(0xcd, 0xd6, 0xf4),
    subtext1: Rgb(0xba, 0xc2, 0xde),
    subtext0: Rgb(0xa6, 0xad, 0xc8),
    overlay2: Rgb(0x93, 0x99, 0xb2),
    overlay1: Rgb(0x7f, 0x84, 0x9c),
    overlay0: Rgb(0x6c, 0x70, 0x86),
    surface2: Rgb(0x58, 0x5b, 0x70),
    surface1: Rgb(0x45, 0x47, 0x5a),
    surface0: Rgb(0x31, 0x32, 0x44),
    base: Rgb(0x1e, 0x1e, 0x2e),
    mantle: Rgb(0x18, 0x18, 0x25),
    crust: Rgb(0x11, 0x11, 0x1b),
};

/// The opacities `catppuccin/nvim` gives its own highlight groups.
///
/// Named rather than written inline so that the relationship between them —
/// `DiffText` being four times `DiffChange`, and stronger than either line
/// colour — is visible in one place.
mod opacity {
    /// `DiffAdd` and `DiffDelete`: a line that changed.
    pub const LINE: u32 = 18;
    /// `DiffText`: the characters within it that actually differ.
    pub const TEXT: u32 = 30;
    /// `DiffChange`: deliberately faint, since a moved block is not an edit.
    pub const MOVED: u32 = 7;
    /// `CursorLine`.
    pub const CURSOR: u32 = 64;
}

/// Derives the interface's colours from a flavour.
pub const fn theme(flavour: Flavour) -> Theme {
    let p = flavour.palette();
    let base = p.base;

    // A `const fn` cannot call `Style`'s builders, which are not const, so the
    // structs are written out. Verbose, but it keeps a theme a compile-time
    // constant rather than something computed at startup.
    const fn on(fg: Rgb, bg: Rgb) -> Style {
        Style::new().fg(colour(fg)).bg(colour(bg))
    }
    const fn over(bg: Rgb) -> Style {
        Style::new().bg(colour(bg))
    }
    const fn ink(fg: Rgb) -> Style {
        Style::new().fg(colour(fg))
    }

    Theme {
        name: flavour.name(),
        dark: base.is_dark(),

        normal: on(p.text, base),

        // A modification is red on the original side and green on the
        // modified one. There is no third "changed" colour, because each side
        // says what happened to *it*.
        deleted: over(blend(p.red, base, opacity::LINE)),
        inserted: over(blend(p.green, base, opacity::LINE)),
        deleted_text: over(blend(p.red, base, opacity::TEXT)),
        inserted_text: over(blend(p.green, base, opacity::TEXT)),
        moved: over(blend(p.blue, base, opacity::MOVED)),

        filler: on(p.surface1, base),
        line_number: ink(p.surface1),
        line_number_current: ink(p.lavender),
        cursor_line: over(blend(p.surface0, base, opacity::CURSOR)),
        divider: on(p.surface0, base),

        status: on(p.text, p.mantle),
        status_path: Style::new().add_modifier(Modifier::BOLD),
        warning: ink(p.red).add_modifier(Modifier::BOLD),
    }
}

/// `Rgb` to `Color` in a const keymap_type, where `From` is unavailable.
const fn colour(Rgb(r, g, b): Rgb) -> ratatui::style::Color {
    ratatui::style::Color::Rgb(r, g, b)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_flavour_derives_a_theme_named_after_itself() {
        for flavour in Flavour::ALL {
            let theme = theme(flavour);
            assert_eq!(theme.name, flavour.name());
            assert!(theme.name.starts_with("catppuccin-"));
        }
    }

    #[test]
    fn only_latte_is_light() {
        for flavour in Flavour::ALL {
            assert_eq!(
                theme(flavour).dark,
                flavour != Flavour::Latte,
                "{:?}",
                flavour
            );
        }
    }

    #[test]
    fn mocha_derives_exactly_the_colours_catppuccin_publishes() {
        // `DiffAdd`, `DiffDelete`, `DiffChange` and `DiffText`, as
        // `catppuccin/nvim` computes them.
        let t = theme(Flavour::Mocha);
        assert_eq!(t.inserted.bg, Some(colour(Rgb(0x36, 0x41, 0x43))));
        assert_eq!(t.deleted.bg, Some(colour(Rgb(0x44, 0x32, 0x44))));
        assert_eq!(t.moved.bg, Some(colour(Rgb(0x25, 0x29, 0x3c))));
        // `DiffText` is blue over base at 30%; ours is the same ratio applied
        // to whichever side's accent, so the *strength* matches even though
        // the hue deliberately does not.
        assert_eq!(
            t.inserted_text.bg,
            Some(colour(blend(MOCHA.green, MOCHA.base, 30)))
        );
    }

    /// The colour a style actually carries, for tests that assert on the
    /// derived theme rather than on the formula that derived it.
    fn background(style: Style) -> Rgb {
        match style.bg {
            Some(ratatui::style::Color::Rgb(r, g, b)) => Rgb(r, g, b),
            other => panic!("a Catppuccin theme must name exact colours, got {other:?}"),
        }
    }

    #[test]
    fn every_flavour_keeps_changed_characters_stronger_than_their_line() {
        // The property the two opacities exist for. Checked on all four,
        // because Latte blends towards white and could easily invert it.
        for flavour in Flavour::ALL {
            let t = theme(flavour);
            let base = flavour.palette().base;
            let distance = |c: Rgb| {
                (c.0 as i32 - base.0 as i32).abs()
                    + (c.1 as i32 - base.1 as i32).abs()
                    + (c.2 as i32 - base.2 as i32).abs()
            };
            for (version, line, text) in [
                ("inserted", t.inserted, t.inserted_text),
                ("deleted", t.deleted, t.deleted_text),
            ] {
                assert!(
                    distance(background(text)) > distance(background(line)),
                    "{flavour:?} {version}: the inner change must stand out \
                     further from the background than the line carrying it"
                );
            }
        }
    }

    #[test]
    fn text_stays_readable_against_every_background_a_row_can_have() {
        for flavour in Flavour::ALL {
            let p = flavour.palette();
            let text = p.text.luma() as i32;
            for (what, bg) in [
                ("base", p.base),
                ("inserted", blend(p.green, p.base, opacity::LINE)),
                ("deleted", blend(p.red, p.base, opacity::LINE)),
                ("inserted text", blend(p.green, p.base, opacity::TEXT)),
                ("deleted text", blend(p.red, p.base, opacity::TEXT)),
                ("cursor line", blend(p.surface0, p.base, opacity::CURSOR)),
            ] {
                let contrast = (text - bg.luma() as i32).abs();
                assert!(
                    contrast > 60,
                    "{flavour:?}: text on {what} has only {contrast} lightness \
                     between them"
                );
            }
        }
    }
}
