//! What the interface is showing, and where the reader is looking at it.
//!
//! ```text
//! buffer/     a sequence of view lines: a diff, a lone file, or the file list
//! viewport.rs one position onto a buffer: top, cursor, horizontal scroll
//! selection.rs a mouse text selection within one column
//! ```
//!
//! Nothing here draws. What a buffer holds is what a reader can move through;
//! how it looks belongs to `components`.

pub mod buffer;
pub mod selection;
mod viewport;

pub use buffer::{Buffer, BufferType, Direction};
pub use viewport::Viewport;
