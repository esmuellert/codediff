//! Test support: a Session with its own event channel for blocking helpers.

use std::rc::Rc;
use std::sync::mpsc::{self, Receiver};
use std::time::Duration;

use channel::{Emitter, Worker};
use file_types::File;
use pipeline::file::{DiffContent, FileWorker};

use crate::app::event::Event;
use crate::app::{Session, Workers};
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

    /// A session over one comparison, as the pipeline would have delivered it.
    pub fn new_diff(content: DiffContent, theme: Theme) -> Self {
        Self::start(theme, None, |session| {
            session.diff = Some(Rc::new(content));
            session.diff_version = syntax::Version(1);
        })
    }

    /// A session over the list of changed files, with nothing open beside it.
    pub fn new_explorer(files: Vec<File>, theme: Theme) -> Self {
        Self::start(theme, None, |session| session.files = Rc::new(files))
    }

    /// The same, with the comparisons the test wants opened waiting in
    /// `script` rather than read from a repository.
    pub fn with_files(
        files: Vec<File>,
        theme: Theme,
        script: Vec<Result<DiffContent, String>>,
    ) -> Self {
        Self::start(theme, Some(script), |session| {
            session.files = Rc::new(files)
        })
    }

    /// Builds the session, fills the store the test starts from, and draws.
    ///
    /// Nothing hands the tree what to show: it builds every frame from the
    /// stores, so a test starting from a fixture puts it where the tree looks.
    fn start(
        theme: Theme,
        script: Option<Vec<Result<DiffContent, String>>>,
        fill: impl FnOnce(&mut Session),
    ) -> Self {
        let (tx, rx) = mpsc::channel();
        let files = match script {
            Some(script) => FileWorker::mock(script, Emitter::new(tx.clone(), Event::FileReady)),
            None => FileWorker::start(Emitter::new(tx.clone(), Event::FileReady)),
        };
        let workers = Workers {
            syntax: Syntax::start(Emitter::new(tx.clone(), Event::Coloured)),
            files,
            list_worker: ListWorker::start(Emitter::new(tx, Event::ListRefreshed)),
            _watcher: None,
        };
        let mut session = Session::new(theme, workers);

        fill(&mut session);

        // The tree must have drawn at a real size before a key that depends
        // on the viewport height (G, PageDown) can have its intended effect.
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
