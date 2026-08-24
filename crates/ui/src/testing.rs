//! Test support: a Session with its own event channel for blocking helpers.

use std::rc::Rc;
use std::sync::Arc;
use std::sync::mpsc::{self, Receiver};
use std::time::Duration;

use channel::{Emitter, Worker};
use pipeline::file::{DiffContent, FileWorker};

use crate::app::event::Event;
use crate::app::{Session, Workers};
use crate::state::{Buffer, BufferType};
use crate::theme::Theme;

use pipeline::list::ListWorker;
use syntax::Syntax;

/// A Session that owns its event channel, for tests that need to block.
pub struct TestSession {
    pub session: Session,
    rx: Receiver<Event>,
}

impl std::ops::Deref for TestSession {
    type Target = Session;
    fn deref(&self) -> &Session {
        &self.session
    }
}

impl std::ops::DerefMut for TestSession {
    fn deref_mut(&mut self) -> &mut Session {
        &mut self.session
    }
}

impl TestSession {
    /// Whether anything on screen is still being coloured.
    pub fn is_colouring(&self) -> bool {
        self.session.workers.syntax.is_busy()
    }

    /// Whether a file comparison is in progress.
    pub fn is_loading_file(&self) -> bool {
        self.session.workers.files.is_busy()
    }

    pub fn new(buffer: Buffer, theme: Theme) -> Self {
        let (tx, rx) = mpsc::channel();
        let workers = Workers {
            syntax: Syntax::start(Emitter::new(tx.clone(), Event::Coloured)),
            files: FileWorker::start(Emitter::new(tx.clone(), Event::FileReady)),
            list_worker: ListWorker::start(Emitter::new(tx, Event::ListRefreshed)),
            _watcher: None,
        };
        let mut session = Session::new(theme, workers);

        fill_stores(&mut session, &buffer);

        // The tree must have drawn at a real size before a key that depends
        // on the viewport height (G, PageDown) can have its intended effect.
        let area = ratatui::layout::Rect::new(0, 0, 80, 24);
        let mut cells = ratatui::buffer::Buffer::empty(area);
        session.draw_into(&mut cells, area);

        Self { session, rx }
    }

    pub fn with_files(
        buffer: Buffer,
        theme: Theme,
        script: Vec<Result<DiffContent, String>>,
    ) -> Self {
        let (tx, rx) = mpsc::channel();
        let workers = Workers {
            syntax: Syntax::start(Emitter::new(tx.clone(), Event::Coloured)),
            files: FileWorker::mock(script, Emitter::new(tx.clone(), Event::FileReady)),
            list_worker: ListWorker::start(Emitter::new(tx, Event::ListRefreshed)),
            _watcher: None,
        };
        let mut session = Session::new(theme, workers);

        fill_stores(&mut session, &buffer);

        let area = ratatui::layout::Rect::new(0, 0, 80, 24);
        let mut cells = ratatui::buffer::Buffer::empty(area);
        session.draw_into(&mut cells, area);

        Self { session, rx }
    }

    /// Blocks until all visible lines are coloured.
    pub fn wait_until_idle(&mut self) -> bool {
        let mut changed = false;
        let mut idle = 0;
        while self.is_colouring() && idle < 8 {
            match self.rx.recv_timeout(Duration::from_secs(5)) {
                Ok(event) => {
                    let applied = self.apply(event);
                    changed |= applied;
                    if applied {
                        idle = 0;
                    } else {
                        idle += 1;
                    }
                }
                Err(_) => break,
            }
        }
        changed
    }

    /// Applies a new file list, as the watcher path does.
    pub fn refresh_list(&mut self, files: Vec<file_types::File>) -> bool {
        self.apply(Event::ListRefreshed(files))
    }

    /// Blocks until a file response arrives.
    pub fn has_file_arrived(&mut self) -> bool {
        loop {
            match self.rx.recv_timeout(Duration::from_secs(5)) {
                Ok(event) => {
                    if matches!(&event, Event::FileReady(_)) {
                        return self.apply(event);
                    }
                    self.apply(event);
                }
                Err(_) => return false,
            }
        }
    }

    /// Applies one worker result and draws, which is what the loop does.
    ///
    /// Without the frame, an effect that answers a new file — clearing the
    /// selection, putting the reader back — would not have run when the test
    /// looks.
    fn apply(&mut self, event: Event) -> bool {
        let changed = self.session.apply(event);
        self.session.settle();
        changed
    }
}

/// Puts what a test's buffer holds into the stores the tree reads from.
///
/// Nothing hands a buffer to the tree: it builds its own from a store, so a
/// test that starts from one has to put what it holds where the tree looks.
fn fill_stores(session: &mut Session, buffer: &Buffer) {
    match buffer.buffer_type() {
        BufferType::SideBySide(side_by_side) => {
            set_diff(session, side_by_side.file(), side_by_side.alignment());
        }
        BufferType::Inline(inline) => {
            set_diff(session, inline.file(), inline.alignment());
        }
        BufferType::SingleFile(single) => {
            // The buffer keeps its lines to itself, so read them back a line
            // at a time; a test's file is short.
            let lines = (0..single.lines())
                .filter_map(|line| single.line(line))
                .map(str::to_owned)
                .collect();
            session
                .diff_store
                .set_content(Some(Rc::new(DiffContent::SingleFile(
                    pipeline::file::SingleFile {
                        file: single.file().clone(),
                        lines: Arc::new(lines),
                    },
                ))));
        }
        BufferType::Explorer(explorer) => {
            // Same again: the list is reachable a row at a time, and a
            // freshly built explorer has every row open.
            let files = (0..explorer.view_lines())
                .filter_map(|line| explorer.file(line))
                .cloned()
                .collect();
            session.file_list_store.fill(files);
        }
    }
}

/// The two-sided case, which side-by-side and inline fill the same way.
fn set_diff(session: &mut Session, file: &file_types::File, alignment: &align::Alignment) {
    session
        .diff_store
        .set_content(Some(Rc::new(DiffContent::Diff(pipeline::file::Diff {
            file: file.clone(),
            alignment: alignment.clone(),
        }))));
}
