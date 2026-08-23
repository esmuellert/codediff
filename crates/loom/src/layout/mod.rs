//! CSS flexbox, in whole cells.

use ratatui::style::Style;

mod flex;

pub(crate) use flex::{Item, assign};

/// CSS flexbox, in whole cells, minus the parts nothing here uses.
///
/// Every field is a flexbox property under its CSS name, so "two `grow: 1`
/// beside one `Length(40)`" has an answer you can look up.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct Layout {
    // As an item of its parent.
    /// `flex-basis` — the size asked for on the parent's main axis, before
    /// growing or shrinking.
    pub basis: Basis,
    /// `flex-grow` — shares of the space left over. 0 takes none.
    pub grow: u16,
    /// `flex-shrink` — shares of the overflow to give back. 0 never shrinks.
    pub shrink: u16,
    /// `min-width` / `min-height`. Nothing shrinks below these, and a parent
    /// that cannot honour them is too small.
    pub min_width: u16,
    pub min_height: u16,
    /// `max-width` / `max-height`. Nothing grows past these.
    pub max_width: Option<u16>,
    pub max_height: Option<u16>,

    // As a container of its children.
    /// `gap` — cells between children.
    pub gap: u16,
    /// `padding` — cells inside the edges, taken off before the children.
    pub pad: Edges,

    // Neither.
    /// Painted before the children. CSS would call this `background`.
    pub fill: Option<Style>,
    /// `overflow: hidden` — children get rectangles no larger than this node's.
    pub clip: bool,
    /// `display: none`, except that the scope and its hooks stay alive:
    /// out of layout, unpainted, unhittable, still remembering.
    pub hidden: bool,
}

/// CSS's defaults: `flex: 0 1 auto`.
impl Default for Layout {
    fn default() -> Self {
        Self {
            basis: Basis::Auto,
            grow: 0,
            shrink: 1,
            min_width: 0,
            min_height: 0,
            max_width: None,
            max_height: None,
            gap: 0,
            pad: Edges::default(),
            fill: None,
            clip: false,
            hidden: false,
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub enum Basis {
    /// As much as the content measures. CSS `flex-basis: auto`.
    #[default]
    Auto,
    /// Exactly this many cells. `Length`, because `Cells` is already this
    /// crate's name for the cell grid.
    Length(u16),
    /// A share of the container's inner size on the main axis.
    Percent(u16),
}

#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub struct Edges {
    pub top: u16,
    pub right: u16,
    pub bottom: u16,
    pub left: u16,
}

impl Edges {
    pub const fn all(n: u16) -> Self {
        Self { top: n, right: n, bottom: n, left: n }
    }
    pub const fn sides(n: u16) -> Self {
        Self { top: 0, right: n, bottom: 0, left: n }
    }
    pub const fn rows(n: u16) -> Self {
        Self { top: n, right: 0, bottom: n, left: 0 }
    }
    pub(crate) const fn across(self) -> u16 {
        self.left + self.right
    }
    pub(crate) const fn down(self) -> u16 {
        self.top + self.bottom
    }
}

/// Which way a container lays its children out.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(crate) enum Axis {
    /// `Row` — children across.
    Across,
    /// `Column` — children down.
    Down,
    /// `Stack` — every child gets the whole rectangle.
    Over,
}
