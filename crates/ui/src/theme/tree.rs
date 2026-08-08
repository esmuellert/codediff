//! What colour a theme gives each part of a tree drawn in rows.
//!
//! ---
//!
//! Taste, and only taste — the same split as [`Code`](super::Code). What a
//! piece of a row *is* — that this one is a directory and that one is a
//! heading — is the view's answer, carried on its nodes. This file says what
//! a directory looks like, which every theme answers differently.
//! `draw::buffer::explorer::node` is where the two meet.
//!
//! **Only what needs rows to mean anything.** An indent guide, a fold arrow, a
//! section title, a count of what is under it — none of these mean anything
//! where nothing nests. What happened to a file does, wherever it is named, so
//! those colours are [`Change`](super::Change) and not here. See D66.
//!
//! **A [`Tree`] holds `Color`, not `Style`.** The background of a row belongs
//! to the selection, and a field that could set one would be able to hide
//! which row the reader is on. Storing a colour rather than a style means a
//! theme *cannot* express that mistake. Bold is not here either: a heading is
//! bold in every theme, so it is structural and lives with the drawing.

use ratatui::style::Color;

use super::catppuccin::Palette;
use super::colour::Rgb;

/// The colour a theme gives each part of a row of a tree.
#[derive(Debug, Clone, Copy)]
pub struct Tree {
    /// A section heading — "Changes", "Staged Changes".
    pub heading: Color,
    /// The indent guides and the fold arrows.
    pub marker: Color,
    pub directory: Color,
    pub name: Color,
    /// Where a file came from, when it moved.
    ///
    /// Not [`Theme::moved`](super::Theme::moved), which is a whole block the
    /// engine judged to have moved within a file. This is a path written
    /// beside a name, and it is faint because the name is what is being read.
    pub previous: Color,
    /// How many files a section holds.
    pub count: Color,
}

impl Tree {
    /// The Catppuccin assignment, for any of its flavours.
    pub const fn catppuccin(p: &Palette) -> Self {
        const fn c(rgb: Rgb) -> Color {
            Color::Rgb(rgb.0, rgb.1, rgb.2)
        }
        Self {
            heading: c(p.lavender),
            marker: c(p.surface1),
            directory: c(p.blue),
            name: c(p.text),
            previous: c(p.overlay1),
            count: c(p.overlay0),
        }
    }
}

/// The same assignment on the sixteen colours every terminal has.
pub const BASIC_DARK: Tree = Tree {
    heading: Color::Cyan,
    marker: Color::DarkGray,
    directory: Color::Blue,
    name: Color::Reset,
    previous: Color::DarkGray,
    count: Color::DarkGray,
};

pub const BASIC_LIGHT: Tree = Tree {
    heading: Color::Blue,
    marker: Color::Gray,
    directory: Color::Blue,
    name: Color::Reset,
    previous: Color::Gray,
    count: Color::Gray,
};
