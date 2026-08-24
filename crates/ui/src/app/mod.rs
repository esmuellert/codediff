//! The event loop: take worker results, wait for a key, dispatch, draw.
//!
//! Session owns the terminal, the workers, the two stores, and the syntax
//! cache. Everything else — the cursor, the scroll, which file is open,
//! the selection — lives inside the component tree.

pub mod event;
pub(crate) mod threads;
mod workers;
pub use workers::Workers;

use std::cell::{Cell, RefCell};
use std::rc::Rc;

use ratatui::backend::Backend;
use ratatui::layout::Rect;

use channel::Worker;
use crate::components::{DiffStore, FileListStore, Root, RootProps};
use crate::terminal::Screen;
use crate::theme::Theme;

/// What the loop should do next.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Flow {
    Continue,
    Suspend,
    Quit,
    #[cfg(debug_assertions)]
    Rebuild,
}

/// Why the loop stopped, for whoever chooses the exit code.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Exit {
    Quit,
    Rebuild,
}

/// One review session.
pub struct Session {
    tree: loom::Tree,
    theme: Theme,
    pub diff_store: DiffStore,
    pub file_list_store: FileListStore,
    pub workers: Workers,
    flow: Rc<Cell<Option<Flow>>>,
    cursor_cell: Rc<Cell<u32>>,
    view_lines_cell: Rc<Cell<u32>>,
}

impl Session {
    pub fn new(theme: Theme, workers: Workers) -> Self {
        let diff_store = DiffStore::new();
        let file_list_store = FileListStore::new();
        let flow = Rc::new(Cell::new(None));

        let flow_cb = {
            let cell = Rc::clone(&flow);
            Rc::new(move |f: Flow| cell.set(Some(f))) as Rc<dyn Fn(Flow)>
        };

        let cursor_cell = Rc::new(Cell::new(0u32));
        let view_lines_cell = Rc::new(Cell::new(0u32));

        let tree = loom::Tree::new::<Root>(RootProps {
            theme: Rc::new(theme),
            diff_store: diff_store.clone(),
            file_list_store: file_list_store.clone(),
            on_flow: flow_cb,
            cursor_cell: Rc::clone(&cursor_cell),
            view_lines_cell: Rc::clone(&view_lines_cell),
        });

        Self { tree, theme, diff_store, file_list_store, workers, flow, cursor_cell, view_lines_cell }
    }

    /// Draws one frame into a cell grid (for tests, without a terminal).
    pub fn draw_into(&mut self, cells: &mut ratatui::buffer::Buffer, area: Rect) {
        self.tree.draw(cells, area);
    }

    pub fn draw<B: Backend>(
        &mut self,
        terminal: &mut ratatui::Terminal<B>,
    ) -> Result<(), B::Error> {
        let tree = &mut self.tree;
        terminal.draw(|frame| {
            let area = frame.area();
            tree.draw(frame.buffer_mut(), area);
        })?;
        Ok(())
    }

    /// The cursor position, as last rendered.
    pub fn cursor(&self) -> u32 {
        self.cursor_cell.get()
    }

    /// The document height in view lines, as last rendered.
    pub fn view_lines(&self) -> u32 {
        self.view_lines_cell.get()
    }

    /// Whether anything needs drawing.
    pub fn needs_draw(&self) -> bool {
        self.tree.needs_draw()
    }

    /// Applies one terminal event — key or mouse.
    pub fn handle_event(&mut self, event: &crossterm::event::Event) -> Flow {
        use crossterm::event::Event;
        match event {
            Event::Key(_) => {
                let Some(key) = crate::input::press(event) else {
                    return Flow::Continue;
                };
                self.tree.press(key);
            }
            Event::Mouse(mouse) => {
                self.tree.mouse(*mouse);
            }
            _ => {}
        }
        self.flow.take().unwrap_or(Flow::Continue)
    }

    /// A key, for tests.
    pub fn press(&mut self, key: crokey::KeyCombination) -> Flow {
        self.tree.press(key);
        self.flow.take().unwrap_or(Flow::Continue)
    }
}

/// The main loop.
pub fn run(
    session: &mut Session,
    repo_root: &std::path::Path,
    tx: std::sync::mpsc::Sender<event::Event>,
    rx: std::sync::mpsc::Receiver<event::Event>,
) -> std::io::Result<Exit> {
    let mut screen = Screen::open()?;
    screen.draw(|t| session.draw(t))?;

    let _input = threads::Input::start(tx.clone());
    #[cfg(unix)]
    let _signals = threads::Signals::start(tx);

    loop {
        let Ok(ev) = rx.recv() else {
            return Ok(Exit::Quit);
        };

        let changed = match ev {
            event::Event::Terminal(ref e) => match session.handle_event(e) {
                Flow::Quit => return Ok(Exit::Quit),
                #[cfg(debug_assertions)]
                Flow::Rebuild => return Ok(Exit::Rebuild),
                Flow::Suspend => {
                    screen.suspend()?;
                    true
                }
                Flow::Continue => true,
            },
            event::Event::FsChanged(_) => {
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
            screen.draw(|t| session.draw(t))?;
        }
    }
}
