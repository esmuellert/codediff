#![doc = include_str!("../README.md")]

pub mod components;
pub mod hooks;
mod keybindings;
pub mod services;
pub mod theme;
mod view;

pub use keybindings::{Action, KeyMap};
pub use theme::{Flavour, Rgb, Theme, blend, catppuccin};

pub use crossterm;
pub use ratatui;

#[cfg(debug_assertions)]
use std::cell::Cell;
use std::path::{Path, PathBuf};
use std::rc::Rc;
use std::sync::mpsc;
#[cfg(unix)]
use std::sync::mpsc::Sender;
#[cfg(unix)]
use std::thread;

use loom::{Flow, Tree, deliver_input};

use components::{App, AppProps};
use services::version_control::VersionControlService;
use services::watcher::WatcherService;

enum Event {
    Terminal(crossterm::event::Event),
    #[cfg(unix)]
    Signal(i32),
    RepositoryChanged(watcher::Refresh),
    /// A background thread changed a file list or file diff.
    Redraw,
}

fn repository_root(cwd: &Path) -> std::io::Result<PathBuf> {
    let repository = vcs::Repository::open(cwd).map_err(std::io::Error::other)?;
    Ok(repository.repo_path().root.clone())
}

pub fn main(cwd: &Path, pathspec: Vec<String>, config: config::Config) -> std::io::Result<i32> {
    let config = Rc::new(config);
    let keybindings =
        Rc::new(KeyMap::from_config(&config.keybindings).map_err(std::io::Error::other)?);
    let diff_settings = pipeline::diff::Settings {
        ignore_trim_whitespace: config.diff.ignore_trim_whitespace,
    };
    let (events_tx, events_rx) = mpsc::channel::<Event>();

    let redraw = events_tx.clone();
    let pipeline = Rc::new(pipeline::Pipeline::start(
        move || {
            let _ = redraw.send(Event::Redraw);
        },
        diff_settings,
        pathspec,
    ));
    let watcher_root = repository_root(cwd)?;
    let _watcher_subscription = watcher::subscribe(
        &watcher_root,
        channel::Emitter::new(events_tx.clone(), Event::RepositoryChanged),
    )
    .map_err(std::io::Error::other)?;

    let version_control_service = Rc::new(VersionControlService::new());
    let watcher_service = Rc::new(WatcherService::new());

    let mut tree = Tree::new::<App>(AppProps {
        cwd: Rc::from(cwd),
        config,
        keybindings,
        diff_service: pipeline,
        version_control_service,
        watcher_service: Rc::clone(&watcher_service),
    });

    #[cfg(unix)]
    spawn_signals(events_tx.clone(), Event::Signal);

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
                watcher_service.deliver(refresh);
                Flow::Continue
            }
            Event::Redraw => {
                tree.redraw_all();
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

/// Delivers process signals through the event loop that owns the terminal.
#[doc(hidden)]
#[cfg(unix)]
pub fn spawn_signals<E: Send + 'static>(events_tx: Sender<E>, wrap: fn(i32) -> E) {
    use signal_hook::consts::{SIGHUP, SIGQUIT, SIGTERM};
    use signal_hook::iterator::Signals;

    let mut signals = Signals::new([SIGTERM, SIGHUP, SIGQUIT]).expect("signal handlers install");
    thread::Builder::new()
        .name("signals".to_owned())
        .spawn(move || {
            for signal in signals.forever() {
                if events_tx.send(wrap(signal)).is_err() {
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

#[cfg(test)]
mod tests {
    use std::process::Command;

    use super::*;

    #[test]
    fn repository_root_is_discovered_from_a_nested_directory() {
        let repo = tempfile::tempdir().unwrap();
        assert!(
            Command::new("git")
                .args(["init", "--quiet"])
                .current_dir(repo.path())
                .status()
                .unwrap()
                .success()
        );
        let nested = repo.path().join("nested/deep");
        std::fs::create_dir_all(&nested).unwrap();

        assert_eq!(
            repository_root(&nested).unwrap().canonicalize().unwrap(),
            repo.path().canonicalize().unwrap()
        );
    }
}
