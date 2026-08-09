//! Talking to the two background threads: one colours text, one reads files.
//!
//! Every method here is either "send a request" or "take what came back", and
//! none of them blocks — the main thread checks on every frame via `try_recv`.

use pipeline::file::Response;

use crate::view::{Buffer, BufferType};

use super::Session;

/// Bail-out for [`Session::wait_until_idle`] if the worker and store disagree.
const IDLE_ANSWERS: u32 = 8;

impl Session {
    /// Blocks until all visible lines are coloured. For tests only.
    pub fn wait_until_idle(&mut self) -> bool {
        let mut changed = false;
        let mut idle = 0;
        while self.is_colouring() && idle < IDLE_ANSWERS {
            let held = self.store.get_cached_lines();
            match self.syntax.next() {
                Some(response) => changed |= self.store.install(response),
                None => break,
            }
            idle = if self.store.get_cached_lines() > held {
                0
            } else {
                idle + 1
            };
            self.send_colour_request();
        }
        changed
    }

    /// Whether anything on screen is still being coloured.
    pub fn is_colouring(&self) -> bool {
        self.syntax.working()
    }

    /// Collects finished syntax spans. Never blocks.
    pub fn receive_colours(&mut self) -> bool {
        let mut changed = false;
        for response in self.syntax.take() {
            changed |= self.store.install(response);
        }
        changed
    }

    /// Asks the syntax worker for anything newly visible.
    pub fn send_colour_request(&mut self) {
        self.view.request(&mut self.syntax, &mut self.store);
    }

    /// Records the list selection for the file worker to pick up.
    pub fn open(&mut self) {
        self.selected = self.selected_file();
    }

    /// Whether a file comparison is in progress.
    pub fn is_loading_file(&self) -> bool {
        self.files.is_busy()
    }

    /// Sends the selected file to the worker if one is pending and the worker
    /// is free.
    pub fn send_file_request(&mut self) {
        if let Some(file) = &self.selected {
            self.files.send_diff_request(file);
        }
    }

    /// Collects a finished file comparison. Never blocks.
    pub fn receive_file(&mut self) -> bool {
        let Some(response) = self.files.poll() else {
            return false;
        };
        if self.selected.as_ref() != Some(&response.file) {
            return false;
        }
        self.selected = None;
        self.apply_file_response(response)
    }

    /// Blocks until the file worker answers. For tests only.
    pub fn has_file_arrived(&mut self) -> bool {
        self.send_file_request();
        let Some(response) = self.files.wait() else {
            return false;
        };
        if self.selected.as_ref() != Some(&response.file) {
            return false;
        }
        self.selected = None;
        self.apply_file_response(response)
    }

    /// Puts a comparison result on screen, or shows the error on the status
    /// line.
    fn apply_file_response(&mut self, response: Response) -> bool {
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
