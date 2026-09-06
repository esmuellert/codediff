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

    /// Colours for added and removed lines.
    pub gained: Color,
    pub lost: Color,
}

impl Change {
    /// Returns the colour for a file change.
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

    /// Catppuccin colours for file changes.
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

/// Basic-terminal colours for file changes.
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

    /// Every declared file-change kind.
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
        // Every theme must distinguish file-change kinds.
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
