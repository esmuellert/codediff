//! One version of a file, shown alone.
//!
//! A peer of [`SideBySide`] and [`Inline`], not a content type. It is what
//! both diff layouts fall back to when a file exists on only one side: there
//! is nothing to lay out against, so neither two columns nor an interleaving
//! has anything to say.
//!
//! No second version means no alignment, no filler and no divider — one column
//! of numbered lines, in the ordinary colours. Nothing here changed *relative
//! to* anything, so nothing is highlighted; marking every line of a new file
//! green says nothing the word "added" does not. VSCode reached the same place
//! and stopped opening a diff editor for added, untracked and deleted files.
//! See D23.
//!
//! It holds no [`Diff`](crate::diff::Diff) for the same reason, which is why
//! that field cannot move up to the parent: an `Option<Diff>` there would be
//! the empty-model trap D23 records.
//!
//! [`SideBySide`]: super::SideBySide
//! [`Inline`]: super::Inline

use file_types::File;

/// One version of a file, and its lines.
#[derive(Debug)]
pub struct SingleFile {
    file: File,
    lines: Vec<String>,
}

impl SingleFile {
    /// Copies the lines in, as [`Alignment::new`] does, so a caller holding
    /// borrowed lines need not convert them first.
    ///
    /// [`Alignment::new`]: align::Alignment::new
    pub fn new(file: File, lines: &[&str]) -> Self {
        Self {
            file,
            lines: lines.iter().map(|line| (*line).to_owned()).collect(),
        }
    }

    /// Which file this is — structured, so a status line can style and shorten
    /// its parts independently.
    pub fn file(&self) -> &File {
        &self.file
    }

    pub fn lines(&self) -> u32 {
        self.lines.len() as u32
    }

    pub fn line(&self, view_line: u32) -> Option<&str> {
        self.lines.get(view_line as usize).map(String::as_str)
    }
}
