//! Turning a view into a grid of cells.
//!
//! Nothing here holds state. A frame is a pure function of the view and the
//! theme, which is why the whole interface can be tested by rendering into a
//! buffer and reading the text back out.
//!
//! **One file renders one thing**, and the files nest the way the things do:
//!
//! ```text
//! screen          the screen: body and status line
//! ├ side_by_side  one pane holding a diff in two columns
//! │ └ column      one gutter-and-text column of a diff
//! │   └ gutter    one line number
//! ├ single_file   one pane holding one version of a file
//! │ └ gutter
//! └ status        the bottom row
//! ```
//!
//! Two files are not renderers, and are used by all of them:
//!
//! ```text
//! layout          where things go — rectangles, and no drawing at all
//! cells           writing characters, tabs and wide glyphs into a rect
//! ```
//!
//! The two pane renderers are named for the buffer kinds they draw, so the two
//! folders line up one for one:
//!
//! ```text
//! view/buffer/side_by_side.rs  ←→  render/side_by_side.rs
//! view/buffer/single_file.rs   ←→  render/single_file.rs
//! ```
//!
//! A buffer kind is dispatched on exactly once, in [`screen`]. Adding one is a
//! new arm and a new file, and the compiler names the arm that is missing.

mod cells;
mod column;
mod gutter;
pub mod layout;
mod screen;
mod side_by_side;
mod single_file;
mod status;

pub use screen::draw;
