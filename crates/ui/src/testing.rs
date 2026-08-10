//! Test support: a Session with its own event channel for blocking helpers.

use std::sync::mpsc::{self, Receiver};
use std::time::Duration;

use channel::Emitter;
use pipeline::file::{DiffContent, FileWorker};

use crate::app::event::Event;
use crate::app::{Session, Workers};
use crate::theme::Theme;
use crate::view::Buffer;

use pipeline::list::ListWorker;
use syntax::{Syntax, SyntaxResponse};

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
    pub fn new(buffer: Buffer, theme: Theme) -> Self {
        let (tx, rx) = mpsc::channel();
        let workers = Workers::spawn(&tx);
        let session = Session::new(buffer, theme, workers);
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
            list_worker: ListWorker::start(Emitter::new(tx, Event::Listed)),
        };
        let session = Session::new(buffer, theme, workers);
        Self { session, rx }
    }

    /// Blocks until all visible lines are coloured.
    pub fn wait_until_idle(&mut self) -> bool {
        let mut changed = false;
        let mut idle = 0;
        while self.session.is_colouring() && idle < 8 {
            match self.rx.recv_timeout(Duration::from_secs(5)) {
                Ok(event) => {
                    let applied = self.session.apply(event);
                    changed |= applied;
                    if applied {
                        idle = 0;
                    } else {
                        idle += 1;
                    }
                    self.session.send_colour_request();
                }
                Err(_) => break,
            }
        }
        changed
    }

    /// Blocks until a file response arrives.
    pub fn has_file_arrived(&mut self) -> bool {
        self.session.send_file_request();
        loop {
            match self.rx.recv_timeout(Duration::from_secs(5)) {
                Ok(event) => {
                    if matches!(&event, Event::FileReady(_)) {
                        return self.session.apply(event);
                    }
                    self.session.apply(event);
                }
                Err(_) => return false,
            }
        }
    }
}
