//! What colour a theme gives each part of a row in the list of changed files.
//!
//! ---
//!
//! Taste, and only taste — the same split as [`Code`](super::Code). What a
//! piece of a row *is* — that this one is a directory and that one is a
//! deletion — is `explorer`'s answer, carried in
//! [`RegionType`](explorer::RegionType). This file says what a deletion looks
//! like, which every theme answers differently.
//!
//! **A [`List`] holds `Color`, not `Style`.** The background of a row belongs
//! to the selection, and a region that could set one would be able to hide
//! which row the reader is on. Storing a colour rather than a style means a
//! theme *cannot* express that mistake. Bold is not here either: a heading is
//! bold in every theme, so it is structural and lives with the drawing.

use ratatui::style::Color;

use super::catppuccin::Palette;
use super::colour::Rgb;

/// The colour a theme gives each part of a row.
#[derive(Debug, Clone, Copy)]
pub struct List {
    /// A section heading — "Changes", "Staged Changes".
    pub heading: Color,
    /// The indent guides and the fold arrows.
    pub marker: Color,
    pub directory: Color,
    pub name: Color,
    /// Where a moved file came from.
    pub moved: Color,
    /// How many files a section holds.
    pub count: Color,
    pub added: Color,
    pub removed: Color,

    /// The letter beside a file, by what happened to it.
    ///
    /// Separate colours rather than one, because this column is what a
    /// reviewer scans: a screen where a deletion and an addition look alike
    /// has to be read a word at a time.
    pub new_file: Color,
    pub modified: Color,
    pub deleted: Color,
    pub renamed: Color,
    pub untracked: Color,
    pub conflicted: Color,
}

impl List {
    /// The Catppuccin assignment, for any of its flavours.
    ///
    /// The letters follow the diff's own colours where they exist — green for
    /// what arrived, red for what went — so the list and the file beside it
    /// agree about what green means.
    pub const fn catppuccin(p: &Palette) -> Self {
        const fn c(rgb: Rgb) -> Color {
            Color::Rgb(rgb.0, rgb.1, rgb.2)
        }
        Self {
            heading: c(p.lavender),
            marker: c(p.surface1),
            directory: c(p.blue),
            name: c(p.text),
            moved: c(p.overlay1),
            count: c(p.overlay0),
            added: c(p.green),
            removed: c(p.red),

            new_file: c(p.green),
            modified: c(p.yellow),
            deleted: c(p.red),
            renamed: c(p.mauve),
            untracked: c(p.teal),
            conflicted: c(p.peach),
        }
    }
}

/// The same assignment on the sixteen colours every terminal has.
///
/// Several land together, as in [`Code`](super::Code): a palette a quarter the
/// size cannot keep six letters apart *and* keep them meaning what they mean.
/// What must stay distinct is added against deleted, which it does.
pub const BASIC_DARK: List = List {
    heading: Color::Cyan,
    marker: Color::DarkGray,
    directory: Color::Blue,
    name: Color::Reset,
    moved: Color::DarkGray,
    count: Color::DarkGray,
    added: Color::Green,
    removed: Color::Red,

    new_file: Color::Green,
    modified: Color::Yellow,
    deleted: Color::Red,
    renamed: Color::Magenta,
    untracked: Color::Cyan,
    conflicted: Color::LightRed,
};

pub const BASIC_LIGHT: List = List {
    heading: Color::Blue,
    marker: Color::Gray,
    directory: Color::Blue,
    name: Color::Reset,
    moved: Color::Gray,
    count: Color::Gray,
    added: Color::Green,
    removed: Color::Red,

    new_file: Color::Green,
    modified: Color::Yellow,
    deleted: Color::Red,
    renamed: Color::Magenta,
    untracked: Color::Cyan,
    conflicted: Color::LightRed,
};
