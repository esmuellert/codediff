//! A diff in two columns.
//!
//! Adds one thing to [`Buffer`](super::Buffer): the divider between the two
//! columns. Everything else about reading a diff — how many view lines, which
//! changed, stepping between them — is the same however it is laid out and
//! lives on the parent.
//!
//! That one field is why this is a type of its own rather than a value of
//! [`Inline`](super::Inline)'s: a divider is meaningless where there are no
//! columns. It is also why it does **not** survive a switch to inline and
//! back — there is nowhere for it to wait, and the alternative is a field
//! `Inline` carries and never reads, at which point the two are the same type
//! and neither name means anything. Pressing `t` is a reader saying they do
//! not want columns; returning to the default split is the answer to that.
//!
//! The divider is here rather than on the pane's `Viewport` because it is not
//! a pane boundary: both columns are inside one pane, drawn by one buffer, so
//! this is the lowest level containing both sides of it. The same rule puts a
//! *pane* border on the tab, one level up. A pane holds only what is true of
//! any view of any buffer, and a percentage meaningless for a lone file is not
//! that. See D27.

use align::Alignment;
use file_types::File;

use crate::diff::Diff;
use crate::highlight::Spans;
use crate::input::{BufferAction, DIVIDER_STEP};

/// The narrowest either column may be squeezed to, in percent.
const MIN_DIVIDER: u16 = 15;
const MAX_DIVIDER: u16 = 85;

/// A diff shown in two columns, and where the line between them sits.
#[derive(Debug)]
pub struct SideBySide {
    diff: Diff,
    /// The share of the width given to the original, in percent.
    divider: u16,
}

impl SideBySide {
    pub fn new(diff: Diff) -> Self {
        Self { diff, divider: 50 }
    }

    /// Hands the diff over, for reading the same one inline.
    pub fn into_diff(self) -> Diff {
        self.diff
    }

    /// The pairing to draw from.
    ///
    /// What callers actually want. Handing out the [`Diff`] instead would make
    /// every one of them take a second hop through it — the pass-through
    /// getter `Alignment` had and lost, for the same reason.
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
