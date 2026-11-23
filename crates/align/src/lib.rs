#![doc = include_str!("../README.md")]
//!
//! The crate performs no IO and stores both input texts in [`Alignment`].

mod alignment;
mod decoration;
mod hunk;
pub mod inline;
mod inner;
mod normalize;
pub mod side_by_side;
mod view_line;
mod viewer_mode;

pub use alignment::{Alignment, DiffVersion, Malformed};
pub use decoration::{CharacterDecoration, LineDecorations};
pub use hunk::{DEFAULT_CONTEXT, Hunk, HunkId, hunks};
pub use inner::{Span, span_on, spans, spans_with_tab_width};
pub use view_line::{ViewLine, ViewLineContent, ViewLineType, blocks, is_well_formed};
pub use viewer_mode::ViewLines;
