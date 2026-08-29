#![doc = include_str!("../README.md")]

pub mod components;
pub mod services;
pub mod theme;

pub use theme::{Flavour, Rgb, Theme, blend, catppuccin};

pub use crossterm;
pub use ratatui;

use std::cell::RefCell;
use std::path::Path;
use std::rc::Rc;
use std::sync::mpsc::{self, Sender};
use std::thread;

use file_types::File;
use loom::{Flow, Tree, deliver_input};

use components::{App, AppProps};
use services::file::FileService;

enum Event {
    Terminal(crossterm::event::Event),
    #[cfg(unix)]
    Signal(i32),
    FsChanged(watcher::Refresh),
    ListRefreshed(Vec<File>),
}

pub fn main(cwd: &Path, pathspec: Vec<String>) -> std::io::Result<()> {
    let (tx, rx) = mpsc::channel::<Event>();

    let list_worker = pipeline::list::ListWorker::start(
        channel::Emitter::new(tx.clone(), Event::ListRefreshed),
    );
    let _watcher = watcher::start(
        cwd,
        channel::Emitter::new(tx.clone(), Event::FsChanged),
    ).ok();

    let file_service = Rc::new(FileService::new(
        Rc::new(RefCell::new(list_worker)),
        pathspec,
    ));

    let mut tree = Tree::new::<App>(AppProps {
        cwd: Rc::from(cwd),
        file_service: Rc::clone(&file_service),
    });

    #[cfg(unix)]
    spawn_signals(tx.clone());

    loom::run(
        &mut tree,
        rx,
        tx,
        Event::Terminal,
        move |event, tree| match event {
            Event::Terminal(ref e) => {
                deliver_input(tree, e);
                Flow::Continue
            }
            #[cfg(unix)]
            Event::Signal(sig) => {
                loom::restore();
                std::process::exit(128 + sig);
            }
            Event::FsChanged(what) => {
                file_service.fs_changed(what);
                Flow::Continue
            }
            Event::ListRefreshed(list) => {
                file_service.deliver(list);
                Flow::Continue
            }
        },
    )
}

#[cfg(unix)]
fn spawn_signals(tx: Sender<Event>) {
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
