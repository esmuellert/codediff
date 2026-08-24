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

use crate::components::{Root, RootProps};
use crate::screen_map::ScreenMap;
use crate::input::Resolver;
use crate::terminal::Screen;
use crate::theme::Theme;
use crate::state::{Buffer, PaneId, View};
use syntax::Store;

/// What the loop should do next.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Flow {
    Continue,
    /// Give the terminal back until the reader brings us forward again.
    Suspend,
    Quit,
    /// Leave, so that a supervisor can rebuild and start us again. Produced
    /// only by a debug build.
    Rebuild,
}

/// Why the loop stopped, for whoever chooses the exit code.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Exit {
    Quit,
    Rebuild,
}

/// One review session — the entire running program state.
pub struct Session {
    pub(crate) view: std::rc::Rc<std::cell::RefCell<View>>,
    theme: Theme,
    /// The mounted interface. One per session, kept between frames because
    /// that is where every component's state lives.
    tree: loom::Tree,
    pub(crate) resolver: Resolver,
    pub(crate) workers: Workers,
    /// The file the reader last selected, waiting to be sent to the worker.
    pub(crate) selected: Option<file_types::File>,
    /// Syntax spans for all open files. Shared, because the interface reads
    /// it while the workers fill it.
    pub(crate) store: std::rc::Rc<std::cell::RefCell<Store>>,
    /// Error from the last file open, cleared on the next key.
    pub(crate) notice: Option<String>,
    /// Where each pane landed on the last frame. Filled by the interface,
    /// read by whoever has to say what is under the mouse.
    screen_map: std::rc::Rc<std::cell::RefCell<ScreenMap>>,
    /// A mouse-down that might become a selection if the user drags.
    pending_selection: Option<PendingSelection>,
}

/// Recorded on mouse-down; promoted to a real Selection on first drag.
#[derive(Debug, Clone, Copy)]
pub(crate) struct PendingSelection {
    pub pane: PaneId,
    pub column: crate::state::selection::SelectionColumn,
    pub anchor: crate::state::selection::Pos,
}

/// Pre-built workers, ready to be handed to Session.
impl Session {
    pub fn new(buffer: Buffer, theme: Theme, mut workers: Workers) -> Self {
        let store = std::rc::Rc::new(std::cell::RefCell::new(Store::new()));
        let mut view = View::single(buffer);
        view.request(&mut workers.syntax, &mut store.borrow_mut());
        let colours = std::rc::Rc::clone(&store);

        let view = std::rc::Rc::new(std::cell::RefCell::new(view));
        let screen_map = std::rc::Rc::new(std::cell::RefCell::new(ScreenMap::default()));
        let mut tree = loom::Tree::new::<Root>(RootProps {
            view: std::rc::Rc::clone(&view),
            notice: None,
            map: std::rc::Rc::clone(&screen_map),
            theme: std::rc::Rc::new(theme),
            colours: std::rc::Rc::clone(&colours),
            syntax_on: true,
        });
        tree.redraw_all();

        Self {
            view,
            theme,
            tree,
            resolver: Resolver::new(),
            workers,
            selected: None,
            store,
            notice: None,
            screen_map,
            pending_selection: None,
        }
    }

    pub fn view(&self) -> std::cell::Ref<'_, View> {
        self.view.borrow()
    }

    pub fn view_mut(&mut self) -> std::cell::RefMut<'_, View> {
        self.view.borrow_mut()
    }

    /// Keys typed but not yet resolved, for a `showcmd` display.
    pub fn pending(&self) -> &Resolver {
        &self.resolver
    }

    /// The screen geometry from the last frame.
    pub fn screen_map(&self) -> std::cell::Ref<'_, crate::screen_map::ScreenMap> {
        self.screen_map.borrow()
    }

    /// Hands the interface everything a frame is a function of.
    fn refresh(&mut self) {
        let syntax_on = self.view.borrow().syntax();
        self.tree.set_props::<Root>(RootProps {
            view: std::rc::Rc::clone(&self.view),
            notice: self.notice.as_deref().map(std::rc::Rc::from),
            map: std::rc::Rc::clone(&self.screen_map),
            theme: std::rc::Rc::new(self.theme),
            colours: std::rc::Rc::clone(&self.store),
            syntax_on,
        });
        self.tree.redraw_all();
    }

    /// Draws one frame into a cell grid (for tests, without a terminal).
    pub fn draw_into(&mut self, cells: &mut ratatui::buffer::Buffer, area: Rect) {
        self.refresh();
        self.tree.draw(cells, area);
    }

    pub fn draw<B: Backend>(
        &mut self,
        terminal: &mut ratatui::Terminal<B>,
    ) -> Result<(), B::Error> {
        self.refresh();
        let tree = &mut self.tree;
        terminal.draw(|frame| {
            let area = frame.area();
            tree.draw(frame.buffer_mut(), area);
        })?;
        Ok(())
    }

    /// How many render-and-layout rounds the last frame took.
    pub fn layout_rounds(&self) -> usize {
        self.tree.layout_rounds()
    }

    /// Records the list selection for the file worker to pick up.
    pub fn open(&mut self) {
        self.selected = self.view.borrow().selected_file().cloned();
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
) -> std::io::Result<Exit> {
    let mut screen = Screen::open()?;
    session.send_file_request();
    screen.draw(|t| session.draw(t))?;

    // Phase 2: start terminal sources after raw mode is on.
    let _input = threads::Input::start(tx.clone());
    #[cfg(unix)]
    let _signals = threads::Signals::start(tx);

    loop {
        let Ok(ev) = rx.recv() else {
            return Ok(Exit::Quit);
        };
        tracing::info!(event = ev.name(), "received");

        // Terminal, signal, and watcher events are handled here.
        // Worker events are dispatched via apply().
        let reason = ev.name();
        let changed = match ev {
            event::Event::Terminal(ref e) => match session.handle_event(e) {
                Flow::Quit => return Ok(Exit::Quit),
                Flow::Rebuild => return Ok(Exit::Rebuild),
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

        session.send_colour_request();
        session.send_file_request();
        if changed {
            tracing::info!(reason, "draw");
            screen.draw(|t| session.draw(t))?;
        }
    }
}
