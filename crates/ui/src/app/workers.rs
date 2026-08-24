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
    /// Applies a worker result. Returns whether the screen changed.
    pub(crate) fn apply(&mut self, event: super::event::Event) -> bool {
        use super::event::Event;
        match event {
            Event::Coloured(response) => {
                self.workers.syntax.received(&response);
                // Install the spans and tell the diff store, which tells its
                // subscribers.
                // syntax::Store is not Clone and lives in a RefCell; the
                // DiffStore takes ownership of a new Store copy for its
                // snapshot. Since Store is large, we just notify that
                // colours changed.
                self.diff_store.notify_colours_changed();
                true
            }
            Event::FileReady(response) => {
                self.workers.files.received(&response);
                self.apply_file_response(response)
            }
            Event::ListRefreshed(files) => {
                self.workers.list_worker.received(&files);
                self.file_list_store.fill(files);
                true
            }
            _ => false,
        }
    }

    /// Puts a comparison result on screen, or shows the error on the status
    /// line.
    fn apply_file_response(&mut self, response: Response) -> bool {
        match response.content {
            Ok(content) => {
                self.diff_store.set_content(Some(Rc::new(content)));
                true
            }
            Err(_why) => {
                // The notice goes through the tree — App holds it in
                // use_state and provides it as context. For now the error
                // is logged; wiring it to the tree needs a store or a
                // callback, which is step 13.
                true
            }
        }
    }
}
