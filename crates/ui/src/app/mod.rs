//! The event loop: take worker results, wait for a key, dispatch, draw.

pub mod event;
pub(crate) mod threads;
mod workers;
pub use workers::Workers;

use std::cell::{Cell, RefCell};
use std::rc::Rc;

use ratatui::backend::Backend;
use ratatui::layout::Rect;

use channel::Worker;
use crate::components::{Observed, Root, RootProps};
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
    pub workers: Workers,
    /// The diff on screen, replaced when a new file is opened.
    pub diff: Option<Rc<pipeline::file::DiffContent>>,
    /// Bumped with every new diff so stale colour responses are refused.
    pub diff_version: syntax::Version,
    /// Syntax colours, shared with the component tree.
    pub colours: Rc<RefCell<syntax::Store>>,
    /// The files this review changes.
    pub files: Rc<Vec<file_types::File>>,
    flow: Rc<Cell<Option<Flow>>>,
    to_compare: Rc<RefCell<Option<file_types::File>>>,
    last_area: Rect,
    observed: Rc<Observed>,
    theme: Rc<Theme>,
    repo: Option<Rc<std::path::Path>>,
}

impl Session {
    pub fn new(theme: Theme, workers: Workers) -> Self {
        let colours = Rc::new(RefCell::new(syntax::Store::new()));
        let files = Rc::new(Vec::new());
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

        let observed = Rc::new(Observed {
            on_flow: Some(flow_cb),
            on_open: Some(open_cb),
            ..Observed::default()
        });

        let theme = Rc::new(theme);

        let tree = loom::Tree::new::<Root>(RootProps {
            theme: Rc::clone(&theme),
            repo: None,
            diff: None,
            diff_version: syntax::Version(0),
            colours: Rc::clone(&colours),
            files: Rc::clone(&files),
            observed: Rc::clone(&observed),
        });

        Self {
            tree, workers, diff: None, diff_version: syntax::Version(0),
            colours, files, flow, to_compare, observed, theme, repo: None,
            last_area: Rect::ZERO,
        }
    }

    /// Pushes the current data down to the root before drawing.
    fn update_props(&mut self) {
        self.tree.set_props::<Root>(RootProps {
            theme: Rc::clone(&self.theme),
            repo: self.repo.clone(),
            diff: self.diff.clone(),
            diff_version: self.diff_version,
            colours: Rc::clone(&self.colours),
            files: Rc::clone(&self.files),
            observed: Rc::clone(&self.observed),
        });
    }

    pub fn draw_into(&mut self, cells: &mut ratatui::buffer::Buffer, area: Rect) {
        self.last_area = area;
        self.update_props();
        self.tree.draw(cells, area);
        self.request_colours();
    }

    pub fn draw<B: Backend>(
        &mut self,
        terminal: &mut ratatui::Terminal<B>,
    ) -> Result<(), B::Error> {
        self.update_props();
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

    pub fn cursor(&mut self) -> u32 {
        self.settle();
        self.observed.cursor.get()
    }

    pub fn view_lines(&self) -> u32 {
        self.observed.view_lines.get()
    }

    pub fn layout(&self) -> file_types::DiffType {
        self.observed.layout.get()
    }

    pub(crate) fn settle(&mut self) {
        if self.last_area.width == 0 {
            return;
        }
        let area = self.last_area;
        let mut cells = ratatui::buffer::Buffer::empty(area);
        self.update_props();
        self.tree.draw(&mut cells, area);
        self.request_colours();
    }

    pub(crate) fn request_colours(&mut self) {
        const MARGIN: u32 = 2_000;

        let Some(content) = self.diff.as_ref() else {
            return;
        };
        let version = self.diff_version;
        let last = self.observed.cursor.get().saturating_add(MARGIN);
        let store = &mut *self.colours.borrow_mut();
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

    pub fn open_file(&mut self, file: file_types::File) {
        self.workers.files.send(file);
    }

    fn send_open_request(&mut self) {
        let file = self.to_compare.borrow_mut().take();
        if let Some(file) = file {
            self.open_file(file);
        }
    }

    pub fn selection(&self) -> Option<crate::components::selection::Selection> {
        *self.observed.selection.borrow()
    }

    pub fn needs_draw(&self) -> bool {
        self.tree.needs_draw()
    }

    pub fn handle_event(&mut self, event: &crossterm::event::Event) -> Flow {
        use crossterm::event::Event;
        if self.last_area.width > 0 && self.last_area.height > 0 {
            let area = self.last_area;
            let mut cells = ratatui::buffer::Buffer::empty(area);
            self.update_props();
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
        self.settle();
        self.send_open_request();
        self.flow.take().unwrap_or(Flow::Continue)
    }

    pub fn press(&mut self, key: crokey::KeyCombination) -> Flow {
        if self.last_area.width > 0 && self.last_area.height > 0 {
            let area = self.last_area;
            let mut cells = ratatui::buffer::Buffer::empty(area);
            self.update_props();
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
