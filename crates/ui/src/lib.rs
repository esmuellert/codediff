#![doc = include_str!("../README.md")]

pub mod components;
mod keybindings;
pub mod services;
pub mod theme;

pub use keybindings::{Action, KeyMap};
pub use theme::{Flavour, Rgb, TerminalProbe, Theme, blend, catppuccin, probe};
pub use vcs::{Commit, RevisionRange};

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
use services::{DiffService, SyntaxService, VcsService, WatcherService};

enum Event {
    Terminal(crossterm::event::Event),
    #[cfg(unix)]
    Signal(i32),
    RepositoryChanged(watcher::Refresh),
    WarmCommits,
    /// A background thread changed a file list, file diff, or syntax spans.
    Redraw,
}

fn open_repository(cwd: &Path, revision_range: &vcs::RevisionRange) -> std::io::Result<PathBuf> {
    let repository = vcs::Repository::open(cwd).map_err(std::io::Error::other)?;
    repository
        .check_revision_range(revision_range)
        .map_err(std::io::Error::other)?;
    Ok(repository.repo_path().root.clone())
}

pub fn main(
    cwd: &Path,
    mut config: config::Config,
    revision_range: vcs::RevisionRange,
) -> std::io::Result<i32> {
    let watcher_root = open_repository(cwd, &revision_range)?;
    let probed = probe();
    if config.ui.theme == "auto" || Theme::named(&config.ui.theme).is_none() {
        config.ui.theme = Theme::resolve(probed, |key| std::env::var(key).ok())
            .name
            .to_owned();
    }
    let config = Rc::new(config);
    let keybindings =
        Rc::new(KeyMap::from_config(&config.keybindings).map_err(std::io::Error::other)?);
    let diff_settings = diff::Settings {
        ignore_trim_whitespace: config.diff.ignore_trim_whitespace,
    };
    let (events_tx, events_rx) = mpsc::channel::<Event>();

    let vcs_redraw = events_tx.clone();
    let vcs = Rc::new(VcsService::start(move || {
        let _ = vcs_redraw.send(Event::Redraw);
    }));
    let diff_redraw = events_tx.clone();
    let diff = Rc::new(DiffService::start(
        move || {
            let _ = diff_redraw.send(Event::Redraw);
        },
        diff_settings,
    ));
    let syntax_redraw = events_tx.clone();
    let syntax = Rc::new(SyntaxService::start(move || {
        let _ = syntax_redraw.send(Event::Redraw);
    }));
    let watcher = Rc::new(
        WatcherService::start(
            &watcher_root,
            channel::Emitter::new(events_tx.clone(), Event::RepositoryChanged),
        )
        .map_err(std::io::Error::other)?,
    );

    let vcs_for_loop = Rc::clone(&vcs);
    let mut tree = Tree::new::<App>(AppProps {
        cwd: Rc::from(cwd),
        revision_range,
        config,
        keybindings,
        vcs,
        diff,
        syntax,
        watcher: Rc::clone(&watcher),
    });

    // Enqueued onto `events_rx` so `loom::run` draws Frame 0 first (queuing
    // `Explorer::list_files` and `RevisionHeader::resolve_commits` onto `"vcs"`),
    // and then queues `warm_commits` behind them.
    let _ = events_tx.send(Event::WarmCommits);

    #[cfg(unix)]
    spawn_signals(events_tx.clone(), Event::Signal);

    #[cfg(debug_assertions)]
    let rebuild = Rc::new(Cell::new(false));
    #[cfg(debug_assertions)]
    let rebuild_flag = Rc::clone(&rebuild);

    let warm_tx = events_tx.clone();
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
                if refresh.head || refresh.refs {
                    vcs_for_loop.invalidate_commits();
                }
                watcher.notify(refresh);
                if refresh.head || refresh.refs {
                    let _ = warm_tx.send(Event::WarmCommits);
                }
                Flow::Continue
            }
            Event::WarmCommits => {
                vcs_for_loop.warm_commits(&watcher_root);
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
            open_repository(&nested, &vcs::RevisionRange::Worktree)
                .unwrap()
                .canonicalize()
                .unwrap(),
            repo.path().canonicalize().unwrap()
        );
    }

    #[test]
    fn invalid_revision_is_rejected_when_opening_repository() {
        let repo = tempfile::tempdir().unwrap();
        assert!(
            Command::new("git")
                .args(["init", "--quiet"])
                .current_dir(repo.path())
                .status()
                .unwrap()
                .success()
        );

        let error = open_repository(
            repo.path(),
            &vcs::RevisionRange::Against("nonexistent-rev".to_owned()),
        )
        .unwrap_err();
        assert!(
            error.to_string().contains("nonexistent-rev"),
            "unexpected error: {error}"
        );
    }
}
