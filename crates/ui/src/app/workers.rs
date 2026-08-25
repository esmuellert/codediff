//! Talking to the background threads: send requests, apply responses.

use std::rc::Rc;

use channel::Worker;
use pipeline::file::{FileWorker, Response};
use pipeline::list::ListWorker;
use syntax::Syntax;

use super::Session;

/// Pre-built workers, ready to be handed to Session.
pub struct Workers {
    pub syntax: Syntax,
    pub files: FileWorker,
    pub list_worker: ListWorker,
    pub _watcher: Option<watcher::Watcher>,
}

impl Session {
    pub(crate) fn apply(&mut self, event: super::event::Event) -> bool {
        use super::event::Event;
        let changed = match event {
            Event::Coloured(response) => {
                self.workers.syntax.received(&response);
                if self.colours.borrow_mut().install(response) {
                    true
                } else {
                    false
                }
            }
            Event::FileReady(response) => {
                self.workers.files.received(&response);
                self.apply_file_response(response)
            }
            Event::ListRefreshed(files) => {
                self.workers.list_worker.received(&files);
                self.files = Rc::new(files);
                true
            }
            _ => false,
        };
        self.request_colours();
        changed
    }

    fn apply_file_response(&mut self, response: Response) -> bool {
        match response.content {
            Ok(content) => {
                self.diff = Some(Rc::new(content));
                self.diff_version = syntax::Version(self.diff_version.0 + 1);
                true
            }
            Err(_why) => {
                true
            }
        }
    }
}
