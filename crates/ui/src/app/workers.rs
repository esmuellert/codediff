//! Talking to the background threads: send requests, apply responses.

use channel::Worker;
use pipeline::file::Response;

use crate::view::{Buffer, BufferType};

use super::Session;
use super::event::Event;

impl Session {
    /// Applies a worker result. Returns whether the screen changed.
    pub fn apply(&mut self, event: Event) -> bool {
        match event {
            Event::Coloured(response) => {
                self.syntax.received(&response);
                self.store.install(response)
            }
            Event::FileReady(response) => {
                self.files.received(&response);
                self.apply_file_response_inner(response)
            }
            Event::Listed(files) => {
                self.list_worker.received(&files);
                self.view.update_explorer(files);
                true
            }
            _ => false,
        }
    }

    /// Asks the syntax worker for anything newly visible.
    pub fn send_colour_request(&mut self) {
        self.view.request(&mut self.syntax, &mut self.store);
    }

    /// Whether anything on screen is still being coloured.
    pub fn is_colouring(&self) -> bool {
        self.syntax.is_busy()
    }

    /// Whether a file comparison is in progress.
    pub fn is_loading_file(&self) -> bool {
        self.files.is_busy()
    }

    /// Records the list selection for the file worker to pick up.
    pub fn open(&mut self) {
        self.selected = self.selected_file();
    }

    /// Sends the selected file to the worker if one is pending and the worker
    /// is free.
    pub fn send_file_request(&mut self) {
        if let Some(file) = &self.selected {
            self.files.send(file.clone());
        }
    }

    /// Puts a comparison result on screen, or shows the error on the status
    /// line.
    fn apply_file_response_inner(&mut self, response: Response) -> bool {
        if self.selected.as_ref() != Some(&response.file) {
            return false;
        }
        self.selected = None;
        let keep = self
            .is_file_shown(&response.file)
            .then(|| self.view.tab().right_pane_buffer())
            .flatten()
            .map(|id| self.view.pane_for(id).viewport.cursor());
        match response.content {
            Ok(content) => {
                self.view.show(Buffer::diff(content));
                if let Some(line) = keep {
                    let id = self.view.tab().right_pane_buffer().expect("just shown");
                    let rows = self.view.buffer(id).view_lines();
                    self.view
                        .pane_mut_for(id)
                        .viewport
                        .place(line.min(rows.saturating_sub(1)), rows);
                }
                self.notice = None;
                self.send_colour_request();
            }
            Err(why) => self.notice = Some(why),
        }
        true
    }

    /// Whether this file is already showing beside the list.
    fn is_file_shown(&self, file: &file_types::File) -> bool {
        let Some(id) = self.view.tab().right_pane_buffer() else {
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
            BufferType::Explorer(explorer) => Some(explorer.file(cursor)?.clone()),
            _ => None,
        }
    }
}
