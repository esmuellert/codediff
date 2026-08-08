//! The event loop: take worker results, wait for a key, dispatch, draw.
//!
//! Nothing here computes a diff, touches git, or colours a line.

use pipeline::file::{Files, Response};
use ratatui::backend::Backend;

use crate::draw;
use crate::input::tab::RESIZE_STEP;
use crate::input::{Action, Command, ProgramAction, Resolution, Resolver, TabAction, ViewAction};
use crate::syntax::{Store, Syntax};
use crate::terminal::Screen;
use crate::theme::Theme;
use crate::view::Buffer;
use crate::view::View;

/// Bail-out for [`Session::settle`] if the worker and store disagree.
const IDLE_ANSWERS: u32 = 8;

/// What the loop should do next.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Flow {
    Continue,
    /// Give the terminal back until the reader brings us forward again.
    Suspend,
    Quit,
}

/// One review session.
pub struct Session {
    view: View,
    theme: Theme,
    resolver: Resolver,
    syntax: Syntax,
    files: Files,
    /// The file the reader last selected, waiting to be sent to the worker.
    selected: Option<file_types::File>,
    /// Syntax spans for all open files.
    store: Store,
    /// Error from the last file open, cleared on the next key.
    notice: Option<String>,
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
        // Asked for before the first frame, so the colours are already on
        // their way while the terminal is still being set up.
        view.request(&mut syntax, &mut store);
        Self {
            view,
            theme,
            resolver: Resolver::new(),
            syntax,
            files,
            selected: None,
            store,
            notice: None,
        }
    }

    /// Blocks until all visible lines are coloured. For tests only.
    pub fn settle(&mut self) -> bool {
        let mut changed = false;
        let mut idle = 0;
        while self.painting() && idle < IDLE_ANSWERS {
            let held = self.store.held();
            match self.syntax.next() {
                Some(response) => changed |= self.store.install(response),
                None => break,
            }
            idle = if self.store.held() > held {
                0
            } else {
                idle + 1
            };
            self.request();
        }
        changed
    }

    /// Whether anything on screen is still being coloured.
    pub fn painting(&self) -> bool {
        self.syntax.working()
    }

    /// Collects finished syntax spans. Never blocks.
    pub fn collect(&mut self) -> bool {
        let mut changed = false;
        for response in self.syntax.take() {
            changed |= self.store.install(response);
        }
        changed
    }

    /// Asks the syntax worker for anything newly visible.
    pub fn request(&mut self) {
        self.view.request(&mut self.syntax, &mut self.store);
    }

    pub fn view(&self) -> &View {
        &self.view
    }

    /// Keys typed but not yet resolved, for a `showcmd` display.
    pub fn pending(&self) -> &Resolver {
        &self.resolver
    }

    /// Draws one frame into a cell grid (for tests, without a terminal).
    pub fn draw_into(&mut self, cells: &mut ratatui::buffer::Buffer, area: ratatui::layout::Rect) {
        draw::render(
            cells,
            area,
            &mut self.view,
            &self.theme,
            &self.store,
            self.notice.as_deref(),
        );
    }

    pub fn draw<B: Backend>(
        &mut self,
        terminal: &mut ratatui::Terminal<B>,
    ) -> Result<(), B::Error> {
        terminal.draw(|frame| {
            let area = frame.area();
            draw::render(
                frame.buffer_mut(),
                area,
                &mut self.view,
                &self.theme,
                &self.store,
                self.notice.as_deref(),
            );
        })?;
        Ok(())
    }

    /// Records the list selection for the file worker to pick up.
    pub fn open(&mut self) {
        self.selected = self.selected_file();
    }

    /// Whether a file comparison is in progress.
    pub fn opening(&self) -> bool {
        self.files.working()
    }

    /// Sends the selected file to the worker if one is pending and the worker is free.
    pub fn request_file(&mut self) {
        if let Some(file) = &self.selected {
            self.files.request(file);
        }
    }

    /// Collects a finished file comparison. Never blocks.
    pub fn collect_file(&mut self) -> bool {
        let Some(response) = self.files.take() else {
            return false;
        };
        // Stale response — the reader moved on. Drop it.
        if self.selected.as_ref() != Some(&response.file) {
            return false;
        }
        self.selected = None;
        self.install(response)
    }

    /// Blocks until the file worker answers. For tests only.
    pub fn opened(&mut self) -> bool {
        self.request_file();
        let Some(response) = self.files.wait() else {
            return false;
        };
        if self.selected.as_ref() != Some(&response.file) {
            return false;
        }
        self.selected = None;
        self.install(response)
    }

    /// Puts a comparison result on screen, or shows the error on the status line.
    fn install(&mut self, response: Response) -> bool {
        // If this file is already showing, keep the cursor position.
        let keep = self
            .showing(&response.file)
            .then(|| self.view.tab().shown())
            .flatten()
            .map(|id| self.view.pane_for(id).viewport.cursor());
        match response.content {
            Ok(content) => {
                self.view.show(Buffer::diff(content));
                if let Some(line) = keep {
                    let id = self.view.tab().shown().expect("just shown");
                    let rows = self.view.buffer(id).view_lines();
                    self.view
                        .pane_mut_for(id)
                        .viewport
                        .jump(line.min(rows.saturating_sub(1)), rows);
                }
                self.notice = None;
                self.request();
            }
            Err(why) => self.notice = Some(why),
        }
        true
    }

    /// Whether this file is already showing beside the list.
    fn showing(&self, file: &file_types::File) -> bool {
        let Some(id) = self.view.tab().shown() else {
            return false;
        };
        self.view.buffer(id).file() == Some(file)
    }

    /// The file the list has selected, if a list has focus.
    fn selected_file(&self) -> Option<file_types::File> {
        let pane = self.view.focused();
        let buffer = self.view.buffer(pane.buffer);
        let cursor = pane.viewport.cursor();
        match buffer.buffer_type() {
            crate::view::BufferType::Explorer(explorer) => Some(explorer.file(cursor)?.clone()),
            _ => None,
        }
    }

    /// Applies one key (for tests — `handle` wraps the crossterm event).
    pub fn press(&mut self, key: crokey::KeyCombination) -> Flow {
        self.notice = None;
        match self.resolver.key(key, self.view.keymap_type()) {
            Resolution::Run(command) => self.dispatch(command),
            Resolution::Pending | Resolution::Cancelled | Resolution::Unbound => Flow::Continue,
        }
    }

    /// Applies one terminal event.
    pub fn handle(&mut self, event: &crossterm::event::Event) -> Flow {
        let Some(key) = crate::input::press(event) else {
            return Flow::Continue;
        };
        self.press(key)
    }

    /// Routes a command to its executor.
    fn dispatch(&mut self, command: Command) -> Flow {
        match command.action {
            Action::Buffer(action) => {
                let count = command.repeat();
                let (buffer, viewport) = self.view.focused_mut();
                buffer.act(action, count, viewport);
                Flow::Continue
            }
            Action::Pane(action) => match action {},
            Action::Tab(TabAction::FocusNext) => {
                self.view.tab_mut().focus_next();
                Flow::Continue
            }
            Action::Tab(action @ (TabAction::WidenLeft | TabAction::NarrowLeft)) => {
                let step = match action {
                    TabAction::WidenLeft => RESIZE_STEP,
                    _ => -RESIZE_STEP,
                };
                let by = i32::from(step).saturating_mul(command.repeat() as i32);
                self.view.tab_mut().resize(by);
                Flow::Continue
            }
            Action::View(ViewAction::ToggleLayout) => {
                self.view.toggle_layout();
                Flow::Continue
            }
            Action::View(ViewAction::ToggleSyntax) => {
                self.view.toggle_syntax();
                Flow::Continue
            }
            Action::View(ViewAction::Open) => {
                let (buffer, viewport) = self.view.focused_mut();
                let cursor = viewport.cursor();
                if buffer.select(cursor) {
                    let lines = buffer.view_lines();
                    viewport.jump(cursor.min(lines.saturating_sub(1)), lines);
                } else {
                    self.open();
                }
                Flow::Continue
            }
            Action::Program(ProgramAction::Quit) => Flow::Quit,
            Action::Program(ProgramAction::Suspend) => Flow::Suspend,
            Action::Program(ProgramAction::Redraw) => Flow::Continue,
        }
    }
}

/// The main loop. Terminal is restored by `Screen`'s `Drop`.
pub fn run(session: &mut Session) -> std::io::Result<()> {
    let mut screen = Screen::open()?;
    session.request_file();
    session.draw(screen.terminal())?;

    loop {
        let coloured = session.collect();
        let compared = session.collect_file();
        if coloured || compared {
            session.request();
            session.request_file();
            session.draw(screen.terminal())?;
        }

        let Some(event) = screen.next_event(session.painting() || session.opening())? else {
            continue;
        };

        match session.handle(&event) {
            Flow::Quit => return Ok(()),
            Flow::Suspend => screen.suspend()?,
            Flow::Continue => {}
        }
        session.request();
        session.request_file();
        session.draw(screen.terminal())?;
    }
}
