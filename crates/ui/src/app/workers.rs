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
        let changed = match event {
            Event::Coloured(response) => {
                self.workers.syntax.received(&response);
                // The store keeps the spans; the notification is separate
                // because a piece for content that has moved on is refused,
                // and refusing one changes nothing on screen.
                if self.diff_store.install_colours(response) {
                    self.diff_store.notify_colours_changed();
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
                self.file_list_store.fill(files);
                true
            }
            _ => false,
        };
        // A comparison that has just arrived is uncoloured, and a piece that
        // has just landed leaves the next one still to ask for. Both are
        // known here, before anything is drawn.
        self.request_colours();
        changed
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
