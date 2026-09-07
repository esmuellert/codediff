//! Colours for tree rows: headings, guides, directories, and names.

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
    /// The previous path shown beside a moved file name.
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

/// Basic-terminal colours for tree rows.
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
