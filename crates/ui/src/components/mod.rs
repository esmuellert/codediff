//! The components the interface is built from.

mod app;
pub mod border;
pub mod code_text;
mod context;
pub mod diff_viewer;
pub mod diff_viewer_container;
pub mod explorer;
pub mod filler;
mod fold;
pub mod gutter;
pub mod inline;
pub mod side_by_side;
pub mod single_file;
mod welcome;

pub(crate) use crate::view::terminal_lines::WrappedViewLine;
pub use crate::view::terminal_lines::{TerminalLine, terminal_line_pairs};

pub use app::{App, AppProps};
pub use context::{Context, Ui, UiProps, UiProvider, UiProviderProps};
pub use explorer::{Explorer, ExplorerProps, letter};
