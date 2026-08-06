//! The list of changed files, as something a pane can show.
//!
//! A thin wrapper. Everything about what is listed, how it nests and what is
//! folded lives in the `explorer` crate, which has no terminal and no
//! repository; this is only what makes it a buffer — a count of rows, and a
//! selection that moves with the cursor.
//!
//! **The cursor is the selection.** There is one number, on the pane's
//! viewport, and the model is told about it before it is asked anything. A
//! second number here would be a second answer to "which row is the reader
//! on", and the two would part company the first time a fold changed the row
//! count.

use explorer::{Entry, Explorer as Model, Groups, Row};

/// The list of changed files.
#[derive(Debug)]
pub struct Explorer {
    model: Model,
}

impl Explorer {
    pub fn new(groups: Groups) -> Self {
        Self {
            model: Model::new(groups),
        }
    }

    pub fn rows(&self) -> &[Row] {
        self.model.rows()
    }

    pub fn view_lines(&self) -> u32 {
        self.model.rows().len() as u32
    }

    /// Points the model at the row the cursor is on.
    ///
    /// Called by whatever is about to act on "the selection", immediately
    /// before it acts, so the two cannot disagree. It used to be called from
    /// the draw path as well, on every frame — a render pass with a side
    /// effect on the model, for a number nothing outside this crate read.
    fn follow(&mut self, cursor: u32) {
        self.model.select(cursor as usize);
    }

    /// Opens or shuts the selected row.
    ///
    /// Returns whether it did, so a key bound to both this and opening a file
    /// can tell which of the two happened.
    pub fn toggle(&mut self, cursor: u32) -> bool {
        self.follow(cursor);
        self.model.toggle()
    }

    /// The file under the cursor, or `None` on a heading or a directory.
    pub fn entry(&self, cursor: u32) -> Option<&Entry> {
        // A read, so the model cannot be told where the cursor is first. The
        // row is looked up here instead, from the same number.
        let row = self.model.rows().get(cursor as usize)?;
        self.model.entry_of(row)
    }

    /// Reshapes the list, keeping the reader on the file they were on.
    ///
    /// Returns the row to land on. A row number means nothing across a
    /// rebuild — the view mode renumbers every row — so the file is named
    /// before and looked up after. A file that is no longer listed leaves the
    /// cursor where it was, clamped.
    pub fn reshape(&mut self, cursor: u32, change: impl FnOnce(&mut Model)) -> u32 {
        let anchor = self.model.anchor(cursor as usize);
        change(&mut self.model);
        let landing = anchor
            .and_then(|anchor| self.model.row_of(&anchor))
            .unwrap_or(cursor as usize);
        let last = self.model.rows().len().saturating_sub(1);
        let landing = landing.min(last) as u32;
        self.model.select(landing as usize);
        landing
    }

    /// The first row a reader can do anything with.
    pub fn first_file(&self) -> u32 {
        self.model.first_file().unwrap_or(0) as u32
    }
}
