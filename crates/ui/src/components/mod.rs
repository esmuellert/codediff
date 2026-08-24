//! The components the interface is built from.

mod app;
mod code_text;
mod context;
mod entry;
mod explorer;
mod filler;
mod gutter;
mod inline;
mod row_styles;
mod side_by_side;
mod single_file;
mod status_line;

pub use app::{App, AppProps};
pub use code_text::{CodeText, CodeTextProps};
pub use context::*;
pub use entry::{Body, Content, Entry, EntryProps, Indent, GroupCounts, Status};
pub use explorer::{Explorer, ExplorerProps, FileList, FileListContext, FileListContextProps};
pub use filler::{Filler, FillerProps};
pub use gutter::{Gutter, GutterProps};
pub use inline::{Inline, InlineProps};
pub use row_styles::{clip_to_line, gutter_width, row_styles};
pub use side_by_side::{SideBySide, SideBySideProps};
pub use single_file::{SingleFile, SingleFileProps};
pub use status_line::{Title, Sidecar, StatusLine, StatusLineProps};
