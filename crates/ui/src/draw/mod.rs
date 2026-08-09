//! Composing render bricks into complete buffer drawings.
//!
//! Nothing here holds state. A frame is a function of the view and the theme.
//!
//! ```text
//! draw/screen.rs   the body, and the status line
//! ├ tab.rs         every pane, and the border
//! ├ pane.rs        one buffer, at one height
//! └ buffer/        what a buffer type looks like
//! ```
//!
//! This half of drawing may name `crate::view`; `render` may not.

mod buffer;
mod pane;
mod screen;
pub mod screen_map;
mod status;
mod tab;

pub use screen::render;

use ratatui::layout::Rect;

use crate::syntax::Store;
use crate::theme::Theme;
use crate::view::selection::SelectionColumn;

/// Text areas produced by drawing a buffer. Used for hit-testing and overlay.
pub(crate) type TextRects = Vec<(SelectionColumn, Rect)>;

/// What every pane of a frame is drawn *with*, as opposed to what it draws.
#[derive(Clone, Copy)]
pub struct Look<'a> {
    pub theme: &'a Theme,
    /// Whether code is coloured by its language.
    pub syntax: bool,
    /// Syntax spans for every open file.
    pub store: &'a Store,
}
