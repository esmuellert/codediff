//! A diff read one version per view line.
//!
//! Holds a [`Diff`] and nothing else — inline needs no state that side-by-side
//! does not, and one thing less (no column divider).

use align::Alignment;
use file_types::File;

use pipeline::file;

use super::colour;
use syntax::{Spans, Store, Syntax, Version};

/// A diff shown one version per view line.
#[derive(Debug)]
pub struct Inline {
    diff: file::Diff,
}

impl Inline {
    pub fn new(diff: file::Diff) -> Self {
        Self { diff }
    }

    /// Hands the diff over, for reading the same one in two columns.
    pub fn into_diff(self) -> file::Diff {
        self.diff
    }

    /// The pairing to draw from.
    pub fn alignment(&self) -> &Alignment {
        &self.diff.alignment
    }

    /// Which file this is — structured, so a status line can style and shorten
    /// its parts independently.
    pub fn file(&self) -> &File {
        &self.diff.file
    }

    pub fn hit_timeout(&self) -> bool {
        self.diff.alignment.hit_timeout()
    }

    /// How each version is coloured, for a frame.
    pub fn spans<'a>(&self, store: &'a Store) -> Spans<'a> {
        colour::spans_diff(&self.diff, store)
    }

    /// Asks for everything up to `last`, on both versions.
    pub fn request(&mut self, syntax: &mut Syntax, store: &mut Store, version: Version, last: u32) {
        colour::request_diff(&self.diff, syntax, store, version, last);
    }
}
