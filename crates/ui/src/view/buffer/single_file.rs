//! Colouring one version of a file, shown alone.
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
//! It holds no [`Diff`] for the same reason, which is why that field cannot
//! move up to the parent: an `Option<Diff>` there would be the empty-model
//! trap D23 records.
//!
//! **The file itself is the pipeline's**, held rather than copied out of. This
//! used to be a struct of the same two fields, built from the answer on
//! arrival and adding nothing to it. See D61.
//!
//! [`SideBySide`]: super::SideBySide
//! [`Inline`]: super::Inline
//! [`Diff`]: pipeline::file::Diff

use file_types::File;
use pipeline::file;

use super::colour;
use crate::syntax::{Spans, Store, Syntax, Version};

/// One version of a file, and what has been coloured of it.
#[derive(Debug)]
pub struct SingleFile {
    read: file::SingleFile,
}

impl SingleFile {
    pub fn new(read: file::SingleFile) -> Self {
        Self { read }
    }

    /// The colouring, for a frame.
    pub fn spans<'a>(&self, store: &'a Store) -> Spans<'a> {
        colour::spans_single_file(&self.read, store)
    }

    /// Asks for everything up to `want`.
    pub fn request(&mut self, syntax: &mut Syntax, store: &mut Store, version: Version, want: u32) {
        colour::request_single_file(&self.read, syntax, store, version, want);
    }

    /// Which file this is — structured, so a status line can style and shorten
    /// its parts independently.
    pub fn file(&self) -> &File {
        &self.read.file
    }

    pub fn lines(&self) -> u32 {
        self.read.lines.len() as u32
    }

    pub fn line(&self, view_line: u32) -> Option<&str> {
        self.read.lines.get(view_line as usize).map(String::as_str)
    }
}
