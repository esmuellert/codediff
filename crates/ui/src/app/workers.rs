//! Talking to the background threads: send requests, apply responses.

use channel::Worker;
use pipeline::file::{FileWorker, Response};
use pipeline::list::ListWorker;
use syntax::Syntax;

use crate::state::Buffer;

use super::Session;

/// Pre-built workers, ready to be handed to Session.
pub struct Workers {
    pub syntax: Syntax,
    pub files: FileWorker,
    pub list_worker: ListWorker,
    pub _watcher: Option<watcher::Watcher>,
}

impl Session {
    /// Asks the syntax worker for anything newly visible.
    pub fn send_colour_request(&mut self) {
        self.view.request(&mut self.workers.syntax, &mut self.store);
    }

    /// Sends the selected file to the worker if one is pending and the worker
    /// is free.
    pub fn send_file_request(&mut self) {
        if let Some(file) = &self.selected {
            self.workers.files.send(file.clone());
        }
    }

    /// Applies a worker result. Returns whether the screen changed.
    pub(crate) fn apply(&mut self, event: super::event::Event) -> bool {
        use super::event::Event;
        use channel::Worker;
        match event {
            Event::Coloured(response) => {
                self.workers.syntax.received(&response);
                self.store.install(response)
            }
            Event::FileReady(response) => {
                self.workers.files.received(&response);
                self.apply_file_response(response)
            }
            Event::ListRefreshed(files) => {
                self.workers.list_worker.received(&files);
                self.view.update_explorer(files);
                true
            }
            _ => false,
        }
    }

    /// Puts a comparison result on screen, or shows the error on the status
    /// line.
    pub(crate) fn apply_file_response(&mut self, response: Response) -> bool {
        if self.selected.as_ref() != Some(&response.file) {
            return false;
        }
        self.selected = None;
        match response.content {
            Ok(content) => {
                self.view.show(Buffer::diff(content));
                self.notice = None;
                self.send_colour_request();
            }
            Err(why) => self.notice = Some(why),
        }
        true
    }
}
