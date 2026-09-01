#![doc = include_str!("../README.md")]

pub mod components;
pub mod hooks;
pub mod services;
pub mod theme;

pub use theme::{Flavour, Rgb, Theme, blend, catppuccin};

pub use crossterm;
pub use ratatui;

use std::cell::{Cell, RefCell};
use std::path::Path;
use std::rc::Rc;
use std::sync::mpsc::{self, Sender};
use std::thread;

use file_types::File;
use loom::{Flow, Tree, deliver_input};

use components::{App, AppProps};
use services::diff::DiffService;
use services::files::FilesService;
use services::syntax::SyntaxService;

enum Event {
    Terminal(crossterm::event::Event),
    #[cfg(unix)]
    Signal(i32),
    FsChanged(watcher::Refresh),
    ListRefreshed(Vec<File>),
    FileReady(Box<pipeline::diff::Response>),
    Coloured(syntax::SyntaxResponse),
}

pub fn main(cwd: &Path, pathspec: Vec<String>) -> std::io::Result<i32> {
    let (tx, rx) = mpsc::channel::<Event>();

    let list_worker = pipeline::files::FilesWorker::start(channel::Emitter::new(
        tx.clone(),
        Event::ListRefreshed,
    ));
    let diff_worker =
        pipeline::diff::DiffWorker::start(channel::Emitter::new(tx.clone(), |response| {
            Event::FileReady(Box::new(response))
        }));
    let syntax_worker = syntax::Syntax::start(channel::Emitter::new(tx.clone(), Event::Coloured));
    let _subscription =
        watcher::subscribe(cwd, channel::Emitter::new(tx.clone(), Event::FsChanged)).ok();

    let file_service = Rc::new(FilesService::new(
        Rc::new(RefCell::new(list_worker)),
        pathspec,
    ));
    let syntax_service = Rc::new(SyntaxService::new(Rc::new(RefCell::new(syntax_worker))));
    let diff_service = Rc::new(DiffService::new(Rc::new(RefCell::new(diff_worker))));

    let mut tree = Tree::new::<App>(AppProps {
        cwd: Rc::from(cwd),
        file_service: Rc::clone(&file_service),
        diff_service: Rc::clone(&diff_service),
        syntax_service: Rc::clone(&syntax_service),
    });

    #[cfg(unix)]
    spawn_signals(tx.clone());

    #[cfg(debug_assertions)]
    let rebuild = Rc::new(Cell::new(false));
    #[cfg(debug_assertions)]
    let rebuild_flag = Rc::clone(&rebuild);

    loom::run(
        &mut tree,
        rx,
        tx,
        Event::Terminal,
        move |event, tree| match event {
            Event::Terminal(ref e) => {
                #[cfg(debug_assertions)]
                if is_f5(e) {
                    rebuild_flag.set(true);
                    return Flow::Quit;
                }
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
            Event::FileReady(response) => {
                diff_service.deliver(*response);
                Flow::Continue
            }
            Event::Coloured(response) => {
                syntax_service.deliver(response);
                Flow::Continue
            }
        },
    )?;

    #[cfg(debug_assertions)]
    if rebuild.get() {
        return Ok(42);
    }
    Ok(0)
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

#[cfg(debug_assertions)]
fn is_f5(event: &crossterm::event::Event) -> bool {
    use crossterm::event::{Event, KeyCode, KeyEventKind};
    matches!(
        event,
        Event::Key(key) if key.code == KeyCode::F(5)
            && matches!(key.kind, KeyEventKind::Press | KeyEventKind::Repeat)
    )
}
