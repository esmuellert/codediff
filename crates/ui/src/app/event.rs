//! The event channel: all sources that can wake the main loop.
//!
//! A reader thread does blocking `crossterm::event::read()` and sends here.
//! A signal thread forwards kill signals. The main loop blocks on `recv()`.

use std::sync::mpsc::Sender;
use std::thread;

use crossterm::event;

/// Something that happened — the main loop reacts to one of these each time
/// it wakes.
pub enum Event {
    /// A key press, mouse click, or resize from the terminal.
    Terminal(event::Event),
    /// A kill signal (SIGTERM, SIGHUP, SIGQUIT). The value is the signal
    /// number.
    #[cfg(unix)]
    Signal(i32),
    /// The watcher detected a change in the repo.
    FsChanged,
}

/// Spawns the terminal input reader thread. Must be called after raw mode is
/// enabled.
pub fn spawn_reader(tx: Sender<Event>) {
    thread::Builder::new()
        .name("input".to_owned())
        .spawn(move || {
            while let Ok(ev) = event::read() {
                if tx.send(Event::Terminal(ev)).is_err() {
                    break;
                }
            }
        })
        .expect("the input thread starts");
}

/// Spawns a thread that waits for kill signals and forwards them as events.
#[cfg(unix)]
pub fn spawn_signals(tx: Sender<Event>) {
    use signal_hook::consts::{SIGHUP, SIGQUIT, SIGTERM};
    use signal_hook::iterator::Signals;

    let mut signals = Signals::new([SIGTERM, SIGHUP, SIGQUIT]).expect("signal handlers install");
    thread::Builder::new()
        .name("signals".to_owned())
        .spawn(move || {
            for sig in signals.forever() {
                if tx.send(Event::Signal(sig)).is_err() {
                    break;
                }
            }
        })
        .expect("the signal thread starts");
}
