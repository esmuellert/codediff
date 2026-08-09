//! Background threads that feed the event channel.
//!
//! Every thread here blocks on a source and sends an Event when it fires.
//! The main loop wakes on any of them via a single `rx.recv()`.

use std::path::Path;
use std::sync::mpsc::Sender;
use std::thread;

use crossterm::event;

use super::event::Event;

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

/// Spawns the watcher and a forwarding thread that bridges its output to the
/// event channel.
pub fn spawn_watcher(repo_root: &Path, tx: Sender<Event>) {
    let root = repo_root.to_owned();
    thread::Builder::new()
        .name("watcher-fwd".to_owned())
        .spawn(move || {
            if let Ok((_watcher, rx_watch)) = watcher::start(&root) {
                for _refresh in rx_watch {
                    if tx.send(Event::FsChanged).is_err() {
                        break;
                    }
                }
            }
        })
        .expect("the watcher-fwd thread starts");
}
