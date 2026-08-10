//! The event vocabulary: what the main loop can wake up to.

use pipeline::file::Response;
use syntax::SyntaxResponse;

/// Something that happened — the main loop reacts to one of these each time
/// it wakes.
#[allow(clippy::large_enum_variant)]
pub enum Event {
    /// A key press, mouse click, or resize from the terminal.
    Terminal(crossterm::event::Event),
    /// A kill signal (SIGTERM, SIGHUP, SIGQUIT).
    #[cfg(unix)]
    Signal(i32),
    /// The watcher detected a change in the repo.
    FsChanged(watcher::Refresh),
    /// The syntax worker coloured a chunk.
    Coloured(SyntaxResponse),
    /// The file worker finished a diff.
    FileReady(Response),
    /// The list worker returned a new file list.
    ListRefreshed(Vec<file_types::File>),
}
