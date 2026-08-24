//! A file shown alone — added, untracked, or deleted.
//!
//! No alignment, no filler, no divider. One column of numbered lines in
//! ordinary colours.

use file_types::File;
use pipeline::file;

use super::colour;
use syntax::{Spans, Store, Syntax, Version};

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

    /// Asks for everything up to `last`.
    pub fn request(&mut self, syntax: &mut Syntax, store: &mut Store, version: Version, last: u32) {
        colour::request_single_file(&self.read, syntax, store, version, last);
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
