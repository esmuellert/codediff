//! What each buffer type looks like.
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
mod status;
mod tab;

pub use screen::render;

use crate::syntax::Store;
use crate::theme::Theme;

/// What every pane of a frame is drawn *with*, as opposed to what it draws.
#[derive(Clone, Copy)]
pub struct Look<'a> {
    pub theme: &'a Theme,
    /// Whether code is coloured by its language.
    pub syntax: bool,
    /// Syntax spans for every open file.
    pub store: &'a Store,
}
