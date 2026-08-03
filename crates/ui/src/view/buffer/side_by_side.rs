//! A diff shown in two columns.
//!
//! A [`Diff`] plus **the row space that reading it in two columns produces**.
//! Those are separate for a reason: a row count is not a fact about a diff, it
//! is a fact about a way of laying one out, and an [`align::Row`] is already a
//! *pair* — one line from each side, or a line against a filler. A different
//! layout of the same diff would number its rows differently, so whatever
//! decides the layout has to be what caches the numbers.
//!
//! Both cost a walk of every row and neither can change while the buffer is
//! open, so both are computed once here:
//!
//! - `rows`, the height of the document in rows rather than lines
//! - `blocks`, the runs of changed rows that `]c` steps through

use std::ops::Range;

use align::{Alignment, RowKind};

use file_types::File;

use crate::diff::Diff;
use crate::input::{BufferAction, DIVIDER_STEP};
use crate::view::Viewport;

/// One diff, laid out in two columns.
#[derive(Debug)]
pub struct SideBySide {
    diff: Diff,
    /// Runs of changed rows, so navigation and the status line cannot
    /// disagree about what counts as a change.
    blocks: Vec<Range<u32>>,
    rows: u32,
    /// Where the divider between the two columns sits: the share of the
    /// width given to the original, in percent.
    ///
    /// Here rather than on the pane's [`Viewport`] because the divider is not
    /// a pane boundary — both columns are inside one pane, drawn by this
    /// buffer — so the buffer is the lowest level containing both sides of it.
    /// The same rule that puts a *pane* border on the tab, one level down. A
    /// pane holds only what is true of any view of any buffer, and a
    /// percentage meaningless for a plain file is not that. See D27.
    divider: u16,
    /// Which way the last change-navigation key went when there was nowhere
    /// left to go.
    ///
    /// Kept so the status line can answer the keypress. Without it `]c` at the
    /// last change does nothing and says nothing, which reads as a broken key
    /// rather than as the end of the file. Cleared by the next key, which is
    /// how vim's echo area behaves — and the reason this needs no clock, which
    /// `ui` is forbidden from having.
    exhausted: Option<Direction>,
}

/// Which way a change-navigation key was pressed.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Direction {
    Next,
    Previous,
}

/// The narrowest either column may be squeezed to, in percent.
const MIN_DIVIDER: u16 = 15;
const MAX_DIVIDER: u16 = 85;

impl SideBySide {
    pub fn new(diff: Diff) -> Self {
        let rows = diff.alignment().row_count();
        let blocks = changed_blocks(diff.alignment());
        Self {
            diff,
            blocks,
            rows,
            divider: 50,
            exhausted: None,
        }
    }

    /// Where the divider sits: the share of the width given to the original,
    /// in percent.
    pub fn divider(&self) -> u16 {
        self.divider
    }

    pub fn alignment(&self) -> &Alignment {
        self.diff.alignment()
    }

    pub fn file(&self) -> &File {
        self.diff.file()
    }

    pub fn hit_timeout(&self) -> bool {
        self.diff.hit_timeout()
    }

    pub fn rows(&self) -> u32 {
        self.rows
    }

    pub fn blocks(&self) -> &[Range<u32>] {
        &self.blocks
    }

    /// Which way the last change-navigation key went with nowhere to go.
    pub fn exhausted(&self) -> Option<Direction> {
        self.exhausted
    }

    /// Which changed block a row falls in, if any.
    pub fn block_at(&self, row: u32) -> Option<usize> {
        self.blocks.iter().position(|b| b.contains(&row))
    }

    pub fn act(&mut self, action: BufferAction, count: u32, view: &mut Viewport) {
        // Any key answers the previous one, so the note lasts exactly until
        // the reader does something else.
        self.exhausted = None;
        match action {
            // Generic arithmetic over a row count, which this buffer supplies.
            // Nothing here is diff-specific.
            BufferAction::Motion(motion) => view.motion(motion, count, self.rows),
            // A motion that has to ask this buffer where to go.
            BufferAction::NextChange => {
                let moved = view.step(count, self.rows, |from| {
                    self.blocks.iter().map(|b| b.start).find(|&r| r > from)
                });
                if !moved {
                    self.exhausted = Some(Direction::Next);
                }
            }
            BufferAction::PrevChange => {
                let moved = view.step(count, self.rows, |from| {
                    self.blocks
                        .iter()
                        .map(|b| b.start)
                        .rev()
                        .find(|&r| r < from)
                });
                if !moved {
                    self.exhausted = Some(Direction::Previous);
                }
            }
            // The divider between the two columns is this buffer's own
            // rendering, not a pane boundary, so it is this buffer's own
            // state — the viewport it was lent is not touched.
            BufferAction::WidenOriginal => {
                self.divider = self.divider.saturating_add(step(count)).min(MAX_DIVIDER);
            }
            BufferAction::NarrowOriginal => {
                self.divider = self.divider.saturating_sub(step(count)).max(MIN_DIVIDER);
            }
        }
    }
}

/// Percentage points a repeat moves the divider, saturating so a large count
/// cannot wrap the width back around.
fn step(count: u32) -> u16 {
    u16::try_from(count.saturating_mul(u32::from(DIVIDER_STEP))).unwrap_or(u16::MAX)
}

/// Runs of adjacent changed rows.
///
/// What `]c` steps through and what the status line counts. Deliberately not
/// [`Alignment::hunks`]: those merge changes within a few lines of each other,
/// which is right for collapsing context and wrong for navigation, since it
/// would make two nearby edits one stop.
fn changed_blocks(alignment: &Alignment) -> Vec<Range<u32>> {
    let mut blocks: Vec<Range<u32>> = Vec::new();
    for (index, row) in alignment.rows().enumerate() {
        let index = index as u32;
        if row.kind == RowKind::Unchanged {
            continue;
        }
        match blocks.last_mut() {
            Some(last) if last.end == index => last.end = index + 1,
            _ => blocks.push(index..index + 1),
        }
    }
    blocks
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::input::Motion;

    /// Two identical files. `ui` may not name the engine — `cargo xtask
    /// lint-arch` forbids it — and nothing here depends on there being a
    /// change, so an empty diff is the honest fixture. What a real diff looks
    /// like is tested from the composition root, which may name every layer.
    fn buffer() -> SideBySide {
        let lines = ["a", "b", "c"];
        let empty = diff_types::LinesDiff {
            changes: Vec::new(),
            moves: Vec::new(),
            hit_timeout: false,
        };
        let file = File::unchanged_path(file_types::RepoPath::new(
            "demo.rs",
            std::path::Path::new("/repo"),
        ));
        SideBySide::new(Diff::new(file, Alignment::new(empty, &lines, &lines)))
    }

    #[test]
    fn the_divider_has_stops_at_both_ends() {
        let mut b = buffer();
        let mut view = Viewport::new();
        b.act(BufferAction::NarrowOriginal, 100, &mut view);
        assert_eq!(b.divider(), MIN_DIVIDER);
        b.act(BufferAction::WidenOriginal, 100, &mut view);
        assert_eq!(b.divider(), MAX_DIVIDER);
    }

    #[test]
    fn dragging_the_divider_leaves_the_viewport_alone() {
        // The divider is this buffer's own rendering, so moving it must not
        // touch the position the pane holds. If these were one field, opening
        // the same diff in two panes would tie their dividers together.
        let mut b = buffer();
        let mut view = Viewport::new();
        view.set_height(10, b.rows());
        b.act(BufferAction::Motion(Motion::Down), 2, &mut view);
        let before = (view.top(), view.cursor(), view.left());
        b.act(BufferAction::WidenOriginal, 3, &mut view);
        assert_eq!((view.top(), view.cursor(), view.left()), before);
        assert_ne!(b.divider(), 50, "the divider did move");
    }
}
