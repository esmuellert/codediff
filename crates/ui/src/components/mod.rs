//! The components the interface is built from.

mod app;
pub mod border;
pub mod cells;
mod context;
pub mod diff_viewer;
pub mod explorer;
pub mod gutter;
pub mod filler;
pub mod code_text;
pub mod side_by_side;
mod welcome;

pub use app::{App, AppProps};
pub use context::{Context, Ui, UiProps, UiProvider, UiProviderProps};
pub use explorer::{Explorer, ExplorerProps, letter};
