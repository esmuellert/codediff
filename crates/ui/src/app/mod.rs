//! The event loop: take worker results, wait for a key, dispatch, draw.
//!
//! ```text
//! mod.rs      Session struct, constructors, draw, run loop, and event router
//! workers.rs  sending to and receiving from the two background threads
//! keys.rs     key press and command dispatch
//! mouse.rs    scroll, click, and hit-testing
//! event.rs    Event enum and reader/signal thread spawning
//! ```
//!
//! Nothing here computes a diff, touches git, or colours a line.

mod event;
mod keys;
mod mouse;
mod threads;
mod workers;

use pipeline::file::Files;
use pipeline::list::ListWorker;
use ratatui::backend::Backend;
use ratatui::layout::Rect;

use crate::draw;
use crate::draw::screen_map::ScreenMap;
use crate::input::Resolver;
use crate::terminal::Screen;
use crate::theme::Theme;
use crate::view::{Buffer, PaneId, View};
use syntax::{Store, Syntax};

/// What the loop should do next.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Flow {
    Continue,
    /// Give the terminal back until the reader brings us forward again.
    Suspend,
    Quit,
}

/// One review session — the entire running program state.
pub struct Session {
    pub(crate) view: View,
    theme: Theme,
    pub(crate) resolver: Resolver,
    pub(crate) syntax: Syntax,
    pub(crate) files: Files,
    pub(crate) list_worker: ListWorker,
    /// The file the reader last selected, waiting to be sent to the worker.
    pub(crate) selected: Option<file_types::File>,
    /// Syntax spans for all open files.
    pub(crate) store: Store,
    /// Error from the last file open, cleared on the next key.
    pub(crate) notice: Option<String>,
    /// Where each pane landed on the last frame.
    screen_map: ScreenMap,
    /// A mouse-down that might become a selection if the user drags.
    pending_selection: Option<PendingSelection>,
}

/// Recorded on mouse-down; promoted to a real Selection on first drag.
#[derive(Debug, Clone, Copy)]
pub(crate) struct PendingSelection {
    pub pane: PaneId,
    pub column: crate::view::selection::SelectionColumn,
    pub anchor: crate::view::selection::Pos,
}

impl Session {
    pub fn new(buffer: Buffer, theme: Theme) -> Self {
        Self::with_files(buffer, theme, Files::start())
    }

    /// For tests: uses a canned file worker instead of git.
    pub fn with_files(buffer: Buffer, theme: Theme, files: Files) -> Self {
        let mut syntax = Syntax::start();
        let mut store = Store::new();
        let mut view = View::single(buffer);
        view.request(&mut syntax, &mut store);
        Self {
            view,
            theme,
            resolver: Resolver::new(),
            syntax,
            files,
            list_worker: ListWorker::start(),
            selected: None,
            store,
            notice: None,
            screen_map: ScreenMap::default(),
            pending_selection: None,
        }
    }

    pub fn view(&self) -> &View {
        &self.view
    }

    /// Keys typed but not yet resolved, for a `showcmd` display.
    pub fn pending(&self) -> &Resolver {
        &self.resolver
    }

    /// The screen geometry from the last frame.
    pub fn screen_map(&self) -> &crate::draw::screen_map::ScreenMap {
        &self.screen_map
    }

    /// Draws one frame into a cell grid (for tests, without a terminal).
    pub fn draw_into(&mut self, cells: &mut ratatui::buffer::Buffer, area: Rect) {
        draw::render(
            cells,
            area,
            &mut self.view,
            &self.theme,
            &self.store,
            self.notice.as_deref(),
            &mut self.screen_map,
        );
    }

    pub fn draw<B: Backend>(
        &mut self,
        terminal: &mut ratatui::Terminal<B>,
    ) -> Result<(), B::Error> {
        let mut screen_map = std::mem::take(&mut self.screen_map);
        terminal.draw(|frame| {
            let area = frame.area();
            draw::render(
                frame.buffer_mut(),
                area,
                &mut self.view,
                &self.theme,
                &self.store,
                self.notice.as_deref(),
                &mut screen_map,
            );
        })?;
        self.screen_map = screen_map;
        Ok(())
    }

    /// Applies one terminal event — key or mouse.
    pub fn handle_event(&mut self, event: &crossterm::event::Event) -> Flow {
        use crossterm::event::Event;
        match event {
            Event::Key(_) => {
                let Some(key) = crate::input::press(event) else {
                    return Flow::Continue;
                };
                self.press(key)
            }
            Event::Mouse(mouse) => self.handle_mouse(mouse),
            _ => Flow::Continue,
        }
    }
}

/// One frame at 60 Hz — how often the loop checks workers while they run.
const FRAME: std::time::Duration = std::time::Duration::from_millis(16);

/// The main loop. Terminal is restored by `Screen`'s `Drop`.
pub fn run(session: &mut Session, repo_root: &std::path::Path) -> std::io::Result<()> {
    use std::sync::mpsc;

    let mut screen = Screen::open()?;
    session.send_file_request();
    session.draw(screen.terminal())?;

    let (tx, rx) = mpsc::channel::<event::Event>();
    threads::spawn_reader(tx.clone());
    #[cfg(unix)]
    threads::spawn_signals(tx.clone());
    threads::spawn_watcher(repo_root, tx);

    loop {
        let busy =
            session.is_colouring() || session.is_loading_file() || session.list_worker.is_busy();
        let ev = if busy {
            rx.recv_timeout(FRAME).ok()
        } else {
            rx.recv().ok()
        };

        // Collect worker results.
        let mut changed = session.receive_colours() | session.receive_file();

        // Check list worker for re-list results.
        if let Some(new_files) = session.list_worker.poll() {
            session.view.update_explorer(new_files);
            changed = true;
        }

        // Handle terminal events.
        if let Some(event::Event::Terminal(ref e)) = ev {
            match session.handle_event(e) {
                Flow::Quit => return Ok(()),
                Flow::Suspend => {
                    screen.suspend()?;
                    changed = true;
                }
                Flow::Continue => changed = true,
            }
        }

        // Watcher event: re-list if the worker is free.
        if let Some(event::Event::FsChanged) = ev {
            tracing::debug!("fs change detected");
            let request = pipeline::list::Request::worktree(repo_root);
            session.list_worker.send_request(request);
        }

        // A kill signal: restore the terminal and exit immediately.
        #[cfg(unix)]
        if let Some(event::Event::Signal(sig)) = ev {
            crate::terminal::restore();
            std::process::exit(128 + sig);
        }

        if changed {
            session.send_colour_request();
            session.send_file_request();
            session.draw(screen.terminal())?;
        }
    }
}
