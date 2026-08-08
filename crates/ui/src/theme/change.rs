//! Colours for each kind of file change (added, deleted, modified, etc).
//!
//! Separate from the file list because these apply anywhere a file is named
//! (status line, tabs, headers). Six distinct colours so the change column
//! is scannable at a glance.

use file_types::ChangeType;
use ratatui::style::Color;

use super::catppuccin::Palette;
use super::colour::Rgb;

/// The colour a theme gives each way a file can have changed.
#[derive(Debug, Clone, Copy)]
pub struct Change {
    /// A file that was not there before.
    pub added: Color,
    pub modified: Color,
    pub deleted: Color,
    /// A file that moved, and kept enough of itself to be recognised.
    pub renamed: Color,
    /// A file git is not tracking at all.
    pub untracked: Color,
    pub conflicted: Color,

    /// Lines gained, and lines lost.
    ///
    /// Here rather than beside the six because they count *within* a change
    /// rather than naming one — but for the same reason: `+4 -1` means the
    /// same wherever it is written.
    pub gained: Color,
    pub lost: Color,
}

impl Change {
    /// The colour for one change.
    ///
    /// Here rather than at each caller, so a seventh kind of change is a
    /// field and one arm rather than a search for everywhere the six were
    /// spelled out.
    pub fn of(&self, change: ChangeType) -> Color {
        match change {
            ChangeType::Added => self.added,
            ChangeType::Modified => self.modified,
            ChangeType::Deleted => self.deleted,
            ChangeType::Moved => self.renamed,
            ChangeType::Untracked => self.untracked,
            ChangeType::Conflicted => self.conflicted,
        }
    }

    /// The Catppuccin assignment, for any of its flavours.
    ///
    /// Green for what arrived and red for what went, following the diff's own
    /// colours, so a file's letter and the file itself agree about what green
    /// means.
    pub const fn catppuccin(p: &Palette) -> Self {
        const fn c(rgb: Rgb) -> Color {
            Color::Rgb(rgb.0, rgb.1, rgb.2)
        }
        Self {
            added: c(p.green),
            modified: c(p.yellow),
            deleted: c(p.red),
            renamed: c(p.mauve),
            untracked: c(p.teal),
            conflicted: c(p.peach),

            gained: c(p.green),
            lost: c(p.red),
        }
    }
}

/// The same assignment on the sixteen colours every terminal has.
///
/// Several land together, as in [`Code`](super::Code): a palette a quarter the
/// size cannot keep six of these apart *and* keep them meaning what they mean.
/// What must stay distinct is added against deleted, which it does.
pub const BASIC_DARK: Change = Change {
    added: Color::Green,
    modified: Color::Yellow,
    deleted: Color::Red,
    renamed: Color::Magenta,
    untracked: Color::Cyan,
    conflicted: Color::LightRed,

    gained: Color::Green,
    lost: Color::Red,
};

pub const BASIC_LIGHT: Change = Change {
    added: Color::Green,
    modified: Color::Yellow,
    deleted: Color::Red,
    renamed: Color::Magenta,
    untracked: Color::Cyan,
    conflicted: Color::LightRed,

    gained: Color::Green,
    lost: Color::Red,
};

#[cfg(test)]
mod tests {
    use super::*;

    /// Every way a file can have changed, so a new one cannot be forgotten
    /// here: adding a variant fails to compile until it is listed.
    const EVERY: [ChangeType; 6] = [
        ChangeType::Added,
        ChangeType::Modified,
        ChangeType::Deleted,
        ChangeType::Moved,
        ChangeType::Untracked,
        ChangeType::Conflicted,
    ];

    #[test]
    fn no_two_changes_look_alike_in_any_theme() {
        // The column of letters is what a reviewer scans, so two changes
        // sharing a colour makes the screen readable only a word at a time.
        // Checked for every theme: this used to be asserted for `basic-dark`
        // alone, and a wrong Catppuccin colour was caught by nothing.
        for theme in crate::theme::Theme::ALL {
            let colours: Vec<Color> = EVERY.iter().map(|&c| theme.change.of(c)).collect();
            for (index, colour) in colours.iter().enumerate() {
                assert!(
                    !colours[index + 1..].contains(colour),
                    "{}: two changes are both {colour:?}",
                    theme.name
                );
            }
        }
    }

    #[test]
    fn what_was_gained_never_looks_like_what_was_lost() {
        for theme in crate::theme::Theme::ALL {
            assert_ne!(
                theme.change.gained, theme.change.lost,
                "{}: `+4` and `-1` are the same colour",
                theme.name
            );
        }
    }
}
