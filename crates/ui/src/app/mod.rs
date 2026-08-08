//! The event loop: take worker results, wait for a key, dispatch, draw.
//!
//! ```text
//! mod.rs      Session struct, constructors, draw, and the run loop
//! workers.rs  sending to and receiving from the two background threads
//! input.rs    keys, mouse, and routing commands to executors
//! ```
//!
//! Nothing here computes a diff, touches git, or colours a line.

mod input;
mod workers;

use pipeline::file::Files;
use ratatui::backend::Backend;
use ratatui::layout::Rect;

use crate::draw;
use crate::input::Resolver;
use crate::syntax::{Store, Syntax};
use crate::terminal::Screen;
use crate::theme::Theme;
use crate::view::{Buffer, PaneId, View};

/// What the loop should do next.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Flow {
    Continue,
    /// Give the terminal back until the reader brings us forward again.
    Suspend,
    Quit,
}

/// Where each pane was drawn, so a mouse click can say which one it hit.
///
/// Updated after every frame, and read only on a click.
#[derive(Debug, Default, Clone)]
struct HitMap {
    panes: Vec<(PaneId, Rect)>,
    body: Rect,
}

/// One review session — the entire running program state.
pub struct Session {
    pub(crate) view: View,
    theme: Theme,
    pub(crate) resolver: Resolver,
    pub(crate) syntax: Syntax,
    pub(crate) files: Files,
    /// The file the reader last selected, waiting to be sent to the worker.
    pub(crate) selected: Option<file_types::File>,
    /// Syntax spans for all open files.
    pub(crate) store: Store,
    /// Error from the last file open, cleared on the next key.
    pub(crate) notice: Option<String>,
    /// Where each pane landed on the last frame.
    hit_map: HitMap,
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
            selected: None,
            store,
            notice: None,
            hit_map: HitMap::default(),
        }
    }

    pub fn view(&self) -> &View {
        &self.view
    }

    /// Keys typed but not yet resolved, for a `showcmd` display.
    pub fn pending(&self) -> &Resolver {
        &self.resolver
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
        );
        self.update_hit_map(area);
    }

    pub fn draw<B: Backend>(
        &mut self,
        terminal: &mut ratatui::Terminal<B>,
    ) -> Result<(), B::Error> {
        let completed = terminal.draw(|frame| {
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
        self.update_hit_map(completed.area);
        Ok(())
    }
}

/// The main loop. Terminal is restored by `Screen`'s `Drop`.
pub fn run(session: &mut Session) -> std::io::Result<()> {
    let mut screen = Screen::open()?;
    session.send_file_request();
    session.draw(screen.terminal())?;

    loop {
        let coloured = session.receive_colours();
        let compared = session.receive_file();
        if coloured || compared {
            session.send_colour_request();
            session.send_file_request();
            session.draw(screen.terminal())?;
        }

        let Some(event) = screen.next_event(session.is_colouring() || session.is_loading_file())?
        else {
            continue;
        };

        match session.handle_event(&event) {
            Flow::Quit => return Ok(()),
            Flow::Suspend => screen.suspend()?,
            Flow::Continue => {}
        }
        session.send_colour_request();
        session.send_file_request();
        session.draw(screen.terminal())?;
    }
}
