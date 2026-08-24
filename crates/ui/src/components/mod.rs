//! The components the interface is built from.

mod app;
mod code_text;
pub mod colour;
mod context;
mod entry;
pub mod explorer;
mod filler;
mod gutter;
mod inline;
mod root;
mod row_styles;
pub mod selection;
mod side_by_side;
mod single_file;
mod status_line;
mod viewport;

pub use app::{App, AppProps, FlowContext, FlowContextProps};
pub use code_text::{CodeText, CodeTextProps};
pub use context::*;
pub use entry::{Body, Entry, EntryProps, Indent, Run, Status, priority};
pub use explorer::{Explorer, ExplorerProps, letter};
pub use filler::{Filler, FillerProps};
pub use gutter::{Gutter, GutterProps};
pub use inline::{Inline, InlineProps};
pub use root::{Root, RootProps};
pub use row_styles::{clip_to_line, gutter_width, row_styles};
pub use side_by_side::{SideBySide, SideBySideProps};
pub use single_file::{SingleFile, SingleFileProps};
pub use status_line::{Title, Sidecar, StatusLine, StatusLineProps};
pub use viewport::Viewport;

/// Which way a change-navigation key was pressed.
///
/// Here rather than in one of the two components that use it: `App` presses
/// the key and `StatusLine` says when it had nowhere to go, and neither owns
/// the other.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Direction {
    Next,
    Previous,
}
