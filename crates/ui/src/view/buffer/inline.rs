//! A diff read one version per view line.
//!
//! Holds a [`Diff`] and nothing else, which is the finding rather than an
//! oversight: reading a diff inline needs no state that reading it in two
//! columns does not, and it needs one thing *less* — there are no columns, so
//! there is no divider between them.
//!
//! What makes inline different from [`SideBySide`](super::SideBySide) is not
//! state but the walk: a change is as tall as both its sides together rather
//! than as tall as the taller one, which is [`DiffLayout::Inline`] in `align`.
//! Being a type of its own is what lets the renderer and the keymap dispatch
//! on a variant the compiler checks, rather than reading a field.
//!
//! [`DiffLayout::Inline`]: align::DiffLayout::Inline

use align::Alignment;
use file_types::File;

use crate::diff::Diff;
use crate::highlight::Spans;

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
    pub fn spans(&self) -> Spans<'_> {
        self.diff.spans()
    }

    /// Colours up to the given line of each version.
    pub fn reach(&mut self, original: u32, modified: u32) {
        self.diff.reach(original, modified);
    }

    /// Whether both versions are coloured as far as the given lines.
    pub fn caught_up(&self, original: u32, modified: u32) -> bool {
        self.diff.caught_up(original, modified)
    }

    /// Colours a little more, and says whether there was anything to do.
    pub fn read_more(&mut self) -> bool {
        self.diff.read_more()
    }
}
