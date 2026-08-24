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
    pub diff_store: DiffStore,
    pub file_list_store: FileListStore,
    pub workers: Workers,
    flow: Rc<Cell<Option<Flow>>>,
    /// The file the interface last chose. A component cannot reach a worker,
    /// so it names the file here and the session sends it.
    to_compare: Rc<RefCell<Option<file_types::File>>>,
    last_area: Rect,
    cursor_cell: Rc<Cell<u32>>,
    view_lines_cell: Rc<Cell<u32>>,
    layout_cell: Rc<Cell<file_types::DiffType>>,
    selection_cell: Rc<RefCell<Option<crate::components::selection::Selection>>>,
    screen_map_cell: Rc<RefCell<crate::screen_map::ScreenMap>>,
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

        let to_compare = Rc::new(RefCell::new(None));
        let open_cb = {
            let cell = Rc::clone(&to_compare);
            Rc::new(move |file: file_types::File| *cell.borrow_mut() = Some(file))
                as Rc<dyn Fn(file_types::File)>
        };

        let cursor_cell = Rc::new(Cell::new(0u32));
        let view_lines_cell = Rc::new(Cell::new(0u32));
        let layout_cell = Rc::new(Cell::new(file_types::DiffType::SideBySide));
        let selection_cell = Rc::new(RefCell::new(None));
        let screen_map_cell = Rc::new(RefCell::new(crate::screen_map::ScreenMap::default()));

        let tree = loom::Tree::new::<Root>(RootProps {
            theme: Rc::new(theme),
            // The session is not told where the repository is yet; the caller
            // that knows will hand it over.
            repo: None,
            diff_store: diff_store.clone(),
            file_list_store: file_list_store.clone(),
            on_flow: flow_cb,
            on_open: open_cb,
            cursor_cell: Rc::clone(&cursor_cell),
            view_lines_cell: Rc::clone(&view_lines_cell),
            layout_cell: Rc::clone(&layout_cell),
            selection_cell: Rc::clone(&selection_cell),
            screen_map_cell: Rc::clone(&screen_map_cell),
        });

        Self {
            tree, diff_store, file_list_store, workers, flow, to_compare,
            cursor_cell, view_lines_cell, layout_cell, selection_cell, screen_map_cell,
            last_area: Rect::ZERO,
        }
    }

    /// Draws one frame into a cell grid (for tests, without a terminal).
    pub fn draw_into(&mut self, cells: &mut ratatui::buffer::Buffer, area: Rect) {
        self.last_area = area;
        self.tree.draw(cells, area);
        self.request_colours();
    }

    pub fn draw<B: Backend>(
        &mut self,
        terminal: &mut ratatui::Terminal<B>,
    ) -> Result<(), B::Error> {
        let tree = &mut self.tree;
        let drawn = std::cell::Cell::new(Rect::ZERO);
        terminal.draw(|frame| {
            let area = frame.area();
            drawn.set(area);
            tree.draw(frame.buffer_mut(), area);
        })?;
        self.last_area = drawn.get();
        self.request_colours();
        Ok(())
    }

    /// The cursor position, after applying any pending state.
    pub fn cursor(&mut self) -> u32 {
        self.settle();
        self.cursor_cell.get()
    }

    /// The document height in view lines, as last rendered.
    pub fn view_lines(&self) -> u32 {
        self.view_lines_cell.get()
    }

    /// Which way the open diff is laid out, as last rendered.
    pub fn layout(&self) -> file_types::DiffType {
        self.layout_cell.get()
    }

    /// Draws into a scratch grid, so that whatever a key changed takes effect,
    /// then asks for the colour the new frame turned out to need.
    ///
    /// A key only marks state dirty; the frame after it is what reads it. A
    /// caller that presses a key and then asks a question needs that frame
    /// without wanting the pixels, and so does the read-ahead, which is a
    /// function of where the cursor ended up.
    pub(crate) fn settle(&mut self) {
        if self.last_area.width == 0 {
            return;
        }
        let area = self.last_area;
        let mut cells = ratatui::buffer::Buffer::empty(area);
        self.tree.draw(&mut cells, area);
        self.request_colours();
    }

    /// Asks the syntax worker for what is on screen and not coloured yet,
    /// plus a margin below it.
    ///
    /// After drawing rather than before, because what is worth colouring is
    /// where the reader has just arrived. The store refuses to ask twice for
    /// the same lines, so the ordinary frame sends nothing.
    pub(crate) fn request_colours(&mut self) {
        // How far past the cursor to read ahead. Enough that scrolling does
        // not outrun it, little enough that opening a very large file does
        // not read all of it.
        const MARGIN: u32 = 2_000;

        let Some(content) = self.diff_store.content() else {
            return;
        };
        let version = self.diff_store.version();
        let last = self.cursor_cell.get().saturating_add(MARGIN);
        let colours = self.diff_store.colours();
        let store = &mut *colours.borrow_mut();
        let syntax = &mut self.workers.syntax;
        match content.as_ref() {
            pipeline::file::DiffContent::Diff(diff) => {
                crate::components::colour::request_diff(diff, syntax, store, version, last);
            }
            pipeline::file::DiffContent::SingleFile(single) => {
                crate::components::colour::request_single_file(
                    single, syntax, store, version, last,
                );
            }
        }
    }

    /// Asks the file worker for a comparison of `file`.
    pub fn open_file(&mut self, file: file_types::File) {
        self.workers.files.send(file);
    }

    /// Sends whatever the interface chose while it was answering an event.
    fn send_open_request(&mut self) {
        let file = self.to_compare.borrow_mut().take();
        if let Some(file) = file {
            self.open_file(file);
        }
    }

    /// The text selection, if any.
    pub fn selection(&self) -> Option<crate::components::selection::Selection> {
        *self.selection_cell.borrow()
    }

    /// Where things landed on screen, for mouse hit-testing.
    pub fn screen_map(&self) -> std::cell::Ref<'_, crate::screen_map::ScreenMap> {
        self.screen_map_cell.borrow()
    }

    /// Whether anything needs drawing.
    pub fn needs_draw(&self) -> bool {
        self.tree.needs_draw()
    }

    /// Applies one terminal event — key or mouse.
    ///
    /// The tree must have drawn at its real size before a key can have its
    /// intended effect, because the viewport's height (set by a layout
    /// effect) determines what Bottom and PageDown mean. The test harness
    /// calls handle_event before draw, so this draws first if it has to.
    pub fn handle_event(&mut self, event: &crossterm::event::Event) -> Flow {
        use crossterm::event::Event;
        // The viewport height comes from a layout effect, so the tree needs
        // to have drawn at the real terminal size before a key that depends
        // on it (PageDown, G, resize) can have its intended effect.
        if self.last_area.width > 0 && self.last_area.height > 0 {
            let area = self.last_area;
            let mut cells = ratatui::buffer::Buffer::empty(area);
            self.tree.draw(&mut cells, area);
        }
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
        // Where the key left the cursor decides what is worth colouring, and
        // only a frame knows that.
        self.settle();
        self.send_open_request();
        self.flow.take().unwrap_or(Flow::Continue)
    }

    /// A key, for tests.
    pub fn press(&mut self, key: crokey::KeyCombination) -> Flow {
        if self.last_area.width > 0 && self.last_area.height > 0 {
            let area = self.last_area;
            let mut cells = ratatui::buffer::Buffer::empty(area);
            self.tree.draw(&mut cells, area);
        }
        self.tree.press(key);
        self.settle();
        self.send_open_request();
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
