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

pub mod event;
mod keys;
mod mouse;
pub(crate) mod threads;
mod workers;
pub use workers::Workers;

use channel::Worker;
use ratatui::backend::Backend;
use ratatui::layout::Rect;

use crate::draw;
use crate::draw::screen_map::ScreenMap;
use crate::input::Resolver;
use crate::terminal::Screen;
use crate::theme::Theme;
use crate::view::{Buffer, PaneId, View};
use syntax::Store;

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
    pub(crate) workers: Workers,
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

/// Pre-built workers, ready to be handed to Session.
impl Session {
    pub fn new(buffer: Buffer, theme: Theme, mut workers: Workers) -> Self {
        let mut store = Store::new();
        let mut view = View::single(buffer);
        view.request(&mut workers.syntax, &mut store);
        Self {
            view,
            theme,
            resolver: Resolver::new(),
            workers,
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

    /// Records the list selection for the file worker to pick up.
    pub fn open(&mut self) {
        self.selected = self.view.selected_file().cloned();
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

/// The main loop. Terminal is restored by `Screen`'s `Drop`.
pub fn run(
    session: &mut Session,
    repo_root: &std::path::Path,
    tx: std::sync::mpsc::Sender<event::Event>,
    rx: std::sync::mpsc::Receiver<event::Event>,
) -> std::io::Result<()> {
    let mut screen = Screen::open()?;
    session.send_file_request();
    screen.draw(|t| session.draw(t))?;

    // Phase 2: start terminal sources after raw mode is on.
    let _input = threads::Input::start(tx.clone());
    #[cfg(unix)]
    let _signals = threads::Signals::start(tx);

    loop {
        let Ok(ev) = rx.recv() else { return Ok(()) };

        // Terminal, signal, and watcher events are handled here.
        // Worker events are dispatched via apply().
        let changed = match ev {
            event::Event::Terminal(ref e) => match session.handle_event(e) {
                Flow::Quit => return Ok(()),
                Flow::Suspend => {
                    screen.suspend()?;
                    true
                }
                Flow::Continue => true,
            },
            event::Event::FsChanged(_refresh) => {
                tracing::debug!("fs change detected");
                session
                    .workers
                    .list_worker
                    .send(pipeline::list::Request::worktree(repo_root));
                false
            }
            #[cfg(unix)]
            event::Event::Signal(sig) => {
                crate::terminal::restore();
                std::process::exit(128 + sig);
            }
            other => session.apply(other),
        };

        if changed {
            session.send_colour_request();
            session.send_file_request();
            screen.draw(|t| session.draw(t))?;
        }
    }
}
