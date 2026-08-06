//! What each buffer type looks like.
//!
//! Nothing here holds state. A frame is a function of the view and the theme,
//! which is why the whole interface can be tested by drawing into a buffer and
//! reading the text back out.
//!
//! **One file draws one buffer type**, and the files line up with the model:
//!
//! ```text
//! view/buffer/diff_buffer.rs  ←→  draw/side_by_side.rs  (DiffType::SideBySide)
//!                             ←→  draw/inline.rs        (DiffType::Inline)
//! view/buffer/single_file.rs  ←→  draw/single_file.rs
//! ```
//!
//! A diff carries the layout it is being read in, and that is what [`screen`]
//! dispatches on. Adding either a buffer type or a layout is a new arm and a
//! new file, and the compiler names the arm that is missing.
//!
//! ```text
//! screen          the screen: body and status line
//! ├ side_by_side  one pane holding a diff in two columns
//! ├ inline        one pane holding a diff one version per view line
//! ├ single_file   one pane holding one version of a file
//! └ status        the bottom row
//! ```
//!
//! **This is the only half of drawing that may name [`crate::view`]**, and
//! `cargo xtask lint-arch` holds the other half to it. The bricks these are
//! built from — rectangles, cells, gutters, columns — live in
//! [`render`](crate::render) and know nothing of buffers, so they can be
//! tested without a model and reused by a buffer type that does not exist yet.
//!
//! Not the same word twice: **render** turns a value into marks, **draw**
//! composes those marks into what a buffer type looks like.

mod explorer;
mod inline;
mod screen;
mod side_by_side;
mod single_file;
mod status;

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
