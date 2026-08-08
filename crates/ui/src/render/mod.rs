//! Putting characters and colour on a cell grid.
//!
//! The bricks. Each file here answers a question about **the terminal**, never
//! about the model: where a rectangle goes, how a line number is written, how
//! one line of text becomes cells when it contains a tab, a double-width
//! character or an escape sequence.
//!
//! ```text
//! layout   where things go — rectangles, and no drawing at all
//! cells    one line of text onto one row of the grid
//! gutter   one line number
//! column   one gutter-and-text column of a diff
//! line     how one line of a diff is coloured
//! ```
//!
//! **A feature must not add a file here.** Every brick above arrived with the
//! terminal itself, before there was a file list. One that arrives *with* a
//! feature is that feature wearing a brick's name, however general it looks —
//! which is how `list.rs` and `fit.rs` came to live here, and why they now
//! live beside the list that is their only caller. See D68.
//!
//! **Nothing here may name [`crate::view`]**, and `cargo xtask lint-arch`
//! refuses it. That is the whole distinction from [`draw`](crate::draw): a
//! brick can be handed a rectangle and some text by anything, so it can be
//! tested without a model and reused by a buffer type that does not exist yet.
//! `draw` is what knows that *a side-by-side diff is two of these columns with
//! a divider between them*.
//!
//! Not the same word twice: **render** turns a value into marks, **draw**
//! composes those marks into what a buffer type looks like. `draw` names
//! `render`, never the other way round.

pub mod cells;
pub mod column;
pub mod gutter;
pub mod layout;
pub mod line;
