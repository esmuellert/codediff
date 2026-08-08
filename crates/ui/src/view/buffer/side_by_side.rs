//! A diff in two columns.
//!
//! Adds one thing to [`Buffer`](super::Buffer): the column divider.

use align::Alignment;
use file_types::File;

use pipeline::file;

use super::colour;
use crate::input::{BufferAction, DIVIDER_STEP};
use crate::syntax::{Spans, Store, Syntax, Version};

/// The narrowest either column may be squeezed to, in percent.
const MIN_DIVIDER: u16 = 15;
const MAX_DIVIDER: u16 = 85;

/// A diff shown in two columns, and where the line between them sits.
#[derive(Debug)]
pub struct SideBySide {
    diff: file::Diff,
    /// The share of the width given to the original, in percent.
    divider: u16,
}

impl SideBySide {
    pub fn new(diff: file::Diff) -> Self {
        Self { diff, divider: 50 }
    }

    /// Hands the diff over, for reading the same one inline.
    pub fn into_diff(self) -> file::Diff {
        self.diff
    }

    /// The pairing to draw from.
    ///
    /// What callers actually last. Handing out the [`Diff`] instead would make
    /// every one of them take a second hop through it — the pass-through
    /// getter `Alignment` had and lost, for the same reason.
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

    /// Where the divider sits: the share of the width given to the original,
    /// in percent.
    pub fn divider(&self) -> u16 {
        self.divider
    }

    /// Moves the divider, which touches no viewport: it is this buffer's own
    /// rendering, not a position within it.
    pub fn drag(&mut self, action: BufferAction, count: u32) {
        match action {
            BufferAction::WidenOriginal => {
                self.divider = self.divider.saturating_add(step(count)).min(MAX_DIVIDER);
            }
            BufferAction::NarrowOriginal => {
                self.divider = self.divider.saturating_sub(step(count)).max(MIN_DIVIDER);
            }
            // Reached only through the two arms above.
            _ => {}
        }
    }
}

/// Percentage points a repeat moves the divider, saturating so a large count
/// cannot wrap the width back around.
fn step(count: u32) -> u16 {
    u16::try_from(count.saturating_mul(u32::from(DIVIDER_STEP))).unwrap_or(u16::MAX)
}
