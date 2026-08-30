//! The components the interface is built from.

mod app;
pub mod border;
pub mod cells;
mod context;
pub mod explorer;
pub mod gutter;
pub mod filler;
pub mod code_text;
mod welcome;

pub use app::{App, AppProps};
pub use context::{Context, Ui, UiProps, UiProvider, UiProviderProps};
pub use explorer::{Explorer, ExplorerProps, letter, scroll_top};
