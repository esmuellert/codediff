//! What each buffer type looks like.
//!
//! Nothing here holds state. A frame is a function of the view and the theme,
//! which is why the whole interface can be tested by drawing into a buffer and
//! reading the text back out.
//!
//! **The files are the same picture as [`view`](crate::view)** — one level
//! contains the next, and each hands the one below a rectangle:
//!
//! ```text
//! view/            View      draw/screen.rs   the body, and the status line
//! ├ tab.rs         Tab       ├ tab.rs         every pane, and the border
//! ├ pane.rs        Pane      ├ pane.rs        one buffer, at one height
//! ├ viewport.rs    Viewport  │                (a position; nothing draws it)
//! └ buffer/        Buffer    └ buffer/        what a buffer type looks like
//! ```
//!
//! `status.rs` is not a level: it is the row beneath the body, drawn by
//! `screen.rs` beside it rather than under it.
//!
//! **This is the only half of drawing that may name [`crate::view`]**, and
//! `cargo xtask lint-arch` holds the other half to it. The bricks these are
//! built from — rectangles, cells, gutters, columns — live in
//! [`render`](crate::render) and know nothing of buffers, so they can be
//! tested without a model and reused by a buffer type that does not exist yet.
//!
//! Not the same word twice: **render** turns a value into marks, **draw**
//! composes those marks into what a buffer type looks like.

mod buffer;
mod pane;
mod screen;
mod status;
mod tab;

pub use screen::render;

use crate::syntax::Store;
use crate::theme::Theme;

/// What every pane of a frame is drawn *with*, as opposed to what it draws.
///
/// Gathered for the same reason [`render::line::Painter`] is one level down:
/// these three travel together through every renderer here, and passing them
/// individually made each take eight arguments in an order nothing checked.
///
/// [`render::line::Painter`]: crate::render::line::Painter
#[derive(Clone, Copy)]
pub struct Look<'a> {
    pub theme: &'a Theme,
    /// Whether code is coloured by its language.
    pub syntax: bool,
    /// Every colour that has arrived, for any file. A renderer looks up the
    /// one it is drawing and takes what is there, which before the answers
    /// arrive is nothing.
    pub store: &'a Store,
}
