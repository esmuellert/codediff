//! RGB colour arithmetic.

use ratatui::style::Color;

/// A fixed 24-bit RGB colour used by blending.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Rgb(pub u8, pub u8, pub u8);

impl From<Rgb> for Color {
    fn from(Rgb(r, g, b): Rgb) -> Self {
        // Always use the computed 24-bit colour.
        Color::Rgb(r, g, b)
    }
}

impl Rgb {
    /// Perceived lightness using standard luma weights.
    pub const fn luma(self) -> u8 {
        ((self.0 as u32 * 299 + self.1 as u32 * 587 + self.2 as u32 * 114) / 1000) as u8
    }

    pub const fn is_dark(self) -> bool {
        self.luma() < 128
    }
}

/// `alpha` percent of `fg` over `bg`. Catppuccin's own formula.
///
/// ```text
/// out = round(alpha × foreground + (1 − alpha) × background)
/// ```
///
/// `const` — runs at compile time.
pub const fn blend(fg: Rgb, bg: Rgb, alpha_percent: u32) -> Rgb {
    const fn channel(fg: u8, bg: u8, a: u32) -> u8 {
        // Integer rounding keeps this usable in a const function.
        (((fg as u32 * a) + (bg as u32 * (100 - a)) + 50) / 100) as u8
    }
    Rgb(
        channel(fg.0, bg.0, alpha_percent),
        channel(fg.1, bg.1, alpha_percent),
        channel(fg.2, bg.2, alpha_percent),
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::theme::catppuccin::MOCHA;

    #[test]
    fn the_blend_reproduces_catppuccins_own_diff_colours() {
        // These values pin the Catppuccin blend formula.
        assert_eq!(blend(MOCHA.green, MOCHA.base, 18), Rgb(0x36, 0x41, 0x43));
        assert_eq!(blend(MOCHA.red, MOCHA.base, 18), Rgb(0x44, 0x32, 0x44));
        assert_eq!(blend(MOCHA.blue, MOCHA.base, 7), Rgb(0x25, 0x29, 0x3c));
        assert_eq!(blend(MOCHA.blue, MOCHA.base, 30), Rgb(0x3e, 0x4b, 0x6b));
    }

    #[test]
    fn the_ends_of_the_blend_are_the_colours_themselves() {
        assert_eq!(blend(MOCHA.green, MOCHA.base, 100), MOCHA.green);
        assert_eq!(blend(MOCHA.green, MOCHA.base, 0), MOCHA.base);
    }

    #[test]
    fn blending_is_monotonic() {
        // Increasing opacity must not reduce the green channel.
        let mut previous = MOCHA.base;
        for alpha in (0..=100).step_by(5) {
            let blended = blend(MOCHA.green, MOCHA.base, alpha);
            assert!(blended.1 >= previous.1, "{alpha}% went backwards");
            previous = blended;
        }
    }

    #[test]
    fn lightness_agrees_with_the_flavours_own_names() {
        assert!(MOCHA.base.is_dark());
        assert!(!crate::theme::catppuccin::LATTE.base.is_dark());
    }
}
