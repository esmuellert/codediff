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
use crate::paint::{Painted, Painter, Spans, Version};

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

    /// Asks the painter for both versions.
    pub fn start_painting(&mut self, painter: &Painter, version: Version) {
        self.diff.start_painting(painter, version);
    }

    /// Whether either side is still waiting for colours.
    pub fn painting(&self) -> bool {
        self.diff.painting()
    }

    /// Installs a piece the painter finished.
    pub fn install(&mut self, painted: Painted) -> bool {
        self.diff.install(painted)
    }
}
