#![doc = include_str!("../README.md")]
//!
//! ---
//!
//! Admission criterion: does this say *which line* appears where, or *what
//! changed* about it? Pairing and grouping only — never what it looks like.
//!
//! This crate performs no IO and holds no copy of either file.

mod alignment;
mod hunk;
mod inner;
mod region;
mod row;

pub use alignment::{Alignment, Malformed, Side};
pub use hunk::{DEFAULT_CONTEXT, Hunk, HunkId, hunks};
pub use inner::{Span, span_on, spans, spans_with_tab_width};
pub use region::{UnchangedRegion, regions};
pub use row::{Row, RowKind, Rows, Slot, is_well_formed, row_count, rows};
