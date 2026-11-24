//! The components the interface is built from.

mod app;
pub mod border;
mod context;
pub mod diff_viewer;
pub mod explorer;
pub mod revision_header;
pub mod revision_picker;
pub mod revisions;

pub use app::{App, AppProps};
pub use context::{Context, DirectorySelection, Ui, UiProps};
pub use diff_viewer::{
    DiffCard, DiffCardProps, DiffCards, DiffCardsProps, DiffHeader, DiffHeaderProps, DiffViewMenu,
    DiffViewMenuProps, DiffViewer, DiffViewerProps, TerminalLine, terminal_line_pairs,
};
pub use explorer::{Explorer, ExplorerProps, letter};
pub use revision_header::{RevisionHeader, RevisionHeaderProps};
pub use revision_picker::{RevisionPicker, RevisionPickerProps};
pub use revisions::RevisionSide;
