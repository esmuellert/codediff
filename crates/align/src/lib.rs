#![doc = include_str!("../README.md")]
//!
//! ---
//!
//! Admission criterion: does this say *which line* appears where, or *what
//! changed* about it? Pairing and grouping only — never what a view line **looks
//! like**. Which view line a file line lands on is admitted, and is why two layouts
//! live here; a style, a width or a cell is not.
//!
//! This crate performs no IO. It does hold the two texts — an [`Alignment`]
//! shares both sides so the thread that colours can be handed them — but it
//! reads neither of them from anywhere.

mod alignment;
mod hunk;
pub mod inline;
mod inner;
mod layout;
mod region;
pub mod side_by_side;
mod view_line;

pub use alignment::{Alignment, DiffVersion, Malformed};
pub use hunk::{DEFAULT_CONTEXT, Hunk, HunkId, hunks};
pub use inner::{Span, span_on, spans, spans_with_tab_width};
pub use layout::ViewLines;
pub use region::{UnchangedRegion, regions};
pub use view_line::{Slot, ViewLine, ViewLineType, blocks, is_well_formed};
