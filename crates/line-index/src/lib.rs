#![doc = include_str!("../README.md")]
//!
//! ---
//!
//!
//! This crate performs no IO.

mod coord;
mod grapheme;
mod line;
mod safe;
mod width;

pub use coord::{ByteOff, CellCol, CharIdx, Utf16Col};
pub use grapheme::{Grapheme, graphemes};
pub use line::LineIndex;
pub use safe::{is_dangerous, picture, visible};
pub use width::{grapheme_width, is_bidi_control, tab_advance};

/// Columns a tab advances to by default, matching most editors.
pub const DEFAULT_TAB_WIDTH: u8 = 4;
