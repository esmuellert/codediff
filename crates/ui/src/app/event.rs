//! The event vocabulary: what the main loop can wake up to.

/// Something that happened — the main loop reacts to one of these each time
/// it wakes.
pub enum Event {
    /// A key press, mouse click, or resize from the terminal.
    Terminal(crossterm::event::Event),
    /// A kill signal (SIGTERM, SIGHUP, SIGQUIT). The value is the signal
    /// number.
    #[cfg(unix)]
    Signal(i32),
    /// The watcher detected a change in the repo.
    FsChanged,
}
