#![doc = include_str!("../README.md")]

pub mod components;
pub mod hooks;
pub mod services;
pub mod theme;

pub use theme::{Flavour, Rgb, Theme, blend, catppuccin};

pub use crossterm;
pub use ratatui;

#[cfg(debug_assertions)]
use std::cell::Cell;
use std::cell::RefCell;
use std::path::Path;
use std::rc::Rc;
use std::sync::mpsc;
#[cfg(unix)]
use std::sync::mpsc::Sender;
#[cfg(unix)]
use std::thread;

use loom::{Flow, Tree, deliver_input};

use components::{App, AppProps};
use services::diff::DiffService;
use services::files::FilesService;
use services::syntax::SyntaxService;
use services::version_control::VersionControlService;

enum Event {
    Terminal(crossterm::event::Event),
    #[cfg(unix)]
    Signal(i32),
    RepositoryChanged(watcher::Refresh),
    FilesReady(pipeline::files::Response),
    DiffReady(Box<pipeline::diff::Response>),
    SyntaxReady(syntax::SyntaxResponse),
}

pub fn main(cwd: &Path, pathspec: Vec<String>) -> std::io::Result<i32> {
    let (events_tx, events_rx) = mpsc::channel::<Event>();

    let files_worker = pipeline::files::FilesWorker::start(channel::Emitter::new(
        events_tx.clone(),
        Event::FilesReady,
    ));
    let diff_worker =
        pipeline::diff::DiffWorker::start(channel::Emitter::new(events_tx.clone(), |response| {
            Event::DiffReady(Box::new(response))
        }));
    let syntax_worker =
        syntax::Syntax::start(channel::Emitter::new(events_tx.clone(), Event::SyntaxReady));
    let _watcher_subscription = watcher::subscribe(
        cwd,
        channel::Emitter::new(events_tx.clone(), Event::RepositoryChanged),
    )
    .ok();

    let files_service = Rc::new(FilesService::new(
        Rc::new(RefCell::new(files_worker)),
        pathspec,
    ));
    let syntax_service = Rc::new(SyntaxService::new(Rc::new(RefCell::new(syntax_worker))));
    let diff_service = Rc::new(DiffService::new(Rc::new(RefCell::new(diff_worker))));
    let version_control_service = Rc::new(VersionControlService::new());

    let mut tree = Tree::new::<App>(AppProps {
        cwd: Rc::from(cwd),
        files_service: Rc::clone(&files_service),
        diff_service: Rc::clone(&diff_service),
        syntax_service: Rc::clone(&syntax_service),
        version_control_service,
    });

    #[cfg(unix)]
    spawn_signals(events_tx.clone());

    #[cfg(debug_assertions)]
    let rebuild = Rc::new(Cell::new(false));
    #[cfg(debug_assertions)]
    let rebuild_flag = Rc::clone(&rebuild);

    loom::run(
        &mut tree,
        events_rx,
        events_tx,
        Event::Terminal,
        move |event, tree| match event {
            Event::Terminal(ref terminal_event) => {
                #[cfg(debug_assertions)]
                if is_f5(terminal_event) {
                    rebuild_flag.set(true);
                    return Flow::Quit;
                }
                deliver_input(tree, terminal_event);
                Flow::Continue
            }
            #[cfg(unix)]
            Event::Signal(signal) => {
                loom::restore();
                std::process::exit(128 + signal);
            }
            Event::RepositoryChanged(refresh) => {
                files_service.fs_changed(refresh);
                Flow::Continue
            }
            Event::FilesReady(response) => {
                files_service.deliver(response);
                Flow::Continue
            }
            Event::DiffReady(response) => {
                diff_service.deliver(*response);
                Flow::Continue
            }
            Event::SyntaxReady(response) => {
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
fn spawn_signals(events_tx: Sender<Event>) {
    use signal_hook::consts::{SIGHUP, SIGQUIT, SIGTERM};
    use signal_hook::iterator::Signals;

    let mut signals = Signals::new([SIGTERM, SIGHUP, SIGQUIT]).expect("signal handlers install");
    thread::Builder::new()
        .name("signals".to_owned())
        .spawn(move || {
            for signal in signals.forever() {
                if events_tx.send(Event::Signal(signal)).is_err() {
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
