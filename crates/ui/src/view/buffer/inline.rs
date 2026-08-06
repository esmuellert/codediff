//! A diff read one version per view line.
//!
//! Holds a [`Diff`] and nothing else, which is the finding rather than an
//! oversight: reading a diff inline needs no state that reading it in two
//! columns does not, and it needs one thing *less* — there are no columns, so
//! there is no divider between them.
//!
//! What makes inline different from [`SideBySide`](super::SideBySide) is not
//! state but the walk: a change is as tall as both its sides together rather
//! than as tall as the taller one, which is [`DiffType::Inline`] in `align`.
//! Being a type of its own is what lets the renderer and the keymap dispatch
//! on a variant the compiler checks, rather than reading a field.
//!
//! [`DiffType::Inline`]: file_types::DiffType::Inline

use align::Alignment;
use file_types::File;

use crate::diff::Diff;
use crate::syntax::{Spans, Store, Syntax, Version};

/// A diff shown one version per view line.
#[derive(Debug)]
pub struct Inline {
    diff: Diff,
}

impl Inline {
    pub fn new(diff: Diff) -> Self {
        Self { diff }
    }

    /// Hands the diff over, for reading the same one in two columns.
    pub fn into_diff(self) -> Diff {
        self.diff
    }

    /// The pairing to draw from.
    pub fn alignment(&self) -> &Alignment {
        self.diff.alignment()
    }

    /// Which file this is — structured, so a status line can style and shorten
    /// its parts independently.
    pub fn file(&self) -> &File {
        self.diff.file()
    }

    pub fn hit_timeout(&self) -> bool {
        self.diff.hit_timeout()
    }

    /// How each version is coloured, for a frame.
    pub fn spans<'a>(&self, store: &'a Store) -> Spans<'a> {
        self.diff.spans(store)
    }

    /// Asks for everything up to `want`, on both versions.
    pub fn request(&mut self, syntax: &mut Syntax, store: &mut Store, version: Version, want: u32) {
        self.diff.request(syntax, store, version, want);
    }
}
