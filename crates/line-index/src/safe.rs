//! Making text safe to put on a terminal.
//!
//! Here rather than in a renderer on purpose. [`width::grapheme_width`] gives
//! control and bidirectional characters one column precisely because they
//! are drawn as a placeholder; if the substitution lived somewhere else, one
//! could change without the other and every column after it would be wrong.
//!
//! [`width::grapheme_width`]: crate::grapheme_width

use crate::width::is_bidi_control;

/// Characters a file must not be allowed to send to the terminal showing it.
///
/// Two families, for the same reason: both let a file decide what the reviewer
/// sees rather than what it contains.
///
/// - Control characters. `ESC` starts a sequence the terminal *obeys* —
///   recolour, move the cursor, erase what is already drawn.
/// - Bidirectional formatting. `U+202E RIGHT-TO-LEFT OVERRIDE` and the
///   isolates reorder a line on screen, so it reads as something other than
///   what it executes. This is the Trojan Source attack, and `char::is_control`
///   does not cover it: those are format characters, category `Cf`, not `Cc`.
pub fn is_dangerous(c: char) -> bool {
    c.is_control() || is_bidi_control(c)
}

/// A printable stand-in of the same width, or the character itself.
pub fn picture(c: char) -> char {
    match c {
        // Unicode Control Pictures: U+2400 draws U+0000, U+2401 draws U+0001,
        // and so on through the C0 range.
        '\u{0}'..='\u{1f}' => char::from_u32(0x2400 + c as u32).unwrap_or('\u{fffd}'),
        '\u{7f}' => '\u{2421}', // DEL has its own picture
        // Neither the C1 controls nor the bidi characters have a picture, and
        // both must occupy a column so the columns after them still line up.
        c if is_dangerous(c) => '\u{fffd}',
        c => c,
    }
}

/// Text with anything the terminal would act on replaced by its [`picture`].
///
/// Returns the input unchanged when there is nothing to do, which is almost
/// always, so ordinary lines cost one scan and no allocation beyond the copy.
pub fn visible(text: &str) -> String {
    if !text.chars().any(is_dangerous) {
        return text.to_owned();
    }
    text.chars().map(picture).collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::grapheme_width;

    #[test]
    fn an_escape_sequence_cannot_reach_the_terminal() {
        let attack = "\u{1b}[31mred";
        let safe = visible(attack);
        assert!(!safe.contains('\u{1b}'));
        assert!(safe.starts_with('\u{241b}'));
    }

    #[test]
    fn a_right_to_left_override_is_replaced() {
        assert_eq!(visible("a\u{202e}b"), "a\u{fffd}b");
    }

    #[test]
    fn ordinary_text_is_untouched() {
        for text in ["hello", "日本語", "e\u{301}", "👨‍👩‍👧"] {
            assert_eq!(visible(text), text);
        }
    }

    #[test]
    fn every_substitution_keeps_the_width_the_measurement_promised() {
        // The reason this module is in this crate. If a picture were ever wider
        // or narrower than what it replaces, every column after it would shift.
        for c in [
            '\u{0}', '\u{1b}', '\u{7f}', '\u{85}', '\u{202e}', '\u{2066}',
        ] {
            let original = grapheme_width(&c.to_string());
            let replaced = grapheme_width(&picture(c).to_string());
            assert_eq!(replaced, original, "{c:?} changed width");
            assert_eq!(replaced, 1, "{c:?} should occupy exactly one column");
        }
    }

    #[test]
    fn a_zero_width_joiner_is_left_alone() {
        // It builds emoji and reorders nothing; replacing it would break
        // legitimate text to no benefit.
        assert!(!is_dangerous('\u{200d}'));
        assert!(!is_dangerous('\u{fe0f}'));
    }
}
