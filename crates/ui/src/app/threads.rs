//! Thread spawning — all background threads start here.
//!
//! Two phases:
//! - `spawn_workers(tx, repo_root)` — workers + watcher, safe before terminal
//! - `Input::start(tx)` / `Signals::start(tx)` — after raw mode is enabled

use std::path::Path;
use std::sync::mpsc::Sender;
use std::thread;

use crossterm::event;

use super::Workers;
use super::event::Event;

// === Workers (phase 1: before Screen::open) ===

/// Spawns all worker threads + watcher with emitters mapped to the event channel.
pub fn spawn_workers(tx: &Sender<Event>, repo_root: &Path) -> Workers {
    use pipeline::file::FileWorker;
    use pipeline::list::ListWorker;
    use syntax::Syntax;

    let watcher_handle = watcher::start(
        repo_root,
        channel::Emitter::new(tx.clone(), Event::FsChanged),
    )
    .ok();

    Workers {
        syntax: Syntax::start(channel::Emitter::new(tx.clone(), Event::Coloured)),
        files: FileWorker::start(channel::Emitter::new(tx.clone(), Event::FileReady)),
        list_worker: ListWorker::start(channel::Emitter::new(tx.clone(), Event::ListRefreshed)),
        _watcher: watcher_handle,
    }
}

// === Terminal sources (phase 2: after Screen::open) ===

/// The terminal input reader. Blocked in `event::read()`.
pub struct Input;

impl Input {
    /// Spawns the input reader thread. Must be called after raw mode is enabled.
    pub fn start(tx: Sender<Event>) -> Self {
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
        Self
    }
}

/// The signal handler.
#[cfg(unix)]
pub struct Signals;

#[cfg(unix)]
impl Signals {
    /// Spawns the signal handler thread.
    pub fn start(tx: Sender<Event>) -> Self {
        use signal_hook::consts::{SIGHUP, SIGQUIT, SIGTERM};
        use signal_hook::iterator::Signals as SigIter;

        let mut signals =
            SigIter::new([SIGTERM, SIGHUP, SIGQUIT]).expect("signal handlers install");
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
        Self
    }
}
