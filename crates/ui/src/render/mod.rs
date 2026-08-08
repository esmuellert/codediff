//! Putting characters and colour on a cell grid.
//!
//! Each file answers a question about the terminal: where a rectangle goes,
//! how a line number is written, how text becomes cells.
//!
//! ```text
//! layout   where things go — rectangles
//! cells    one line of text onto one row of the grid
//! gutter   one line number
//! column   one gutter-and-text column of a diff
//! line     how one line of a diff is coloured
//! ```
//!
//! Nothing here may name `crate::view` (`lint-arch` enforces this).
//! `render` turns values into marks; `draw` composes them into what a buffer
//! type looks like.

pub mod cells;
pub mod column;
pub mod gutter;
pub mod layout;
pub mod line;
