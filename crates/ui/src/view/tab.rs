//! An independent arrangement of panes.
//!
//! A tab owns its panes and decides where each one goes. That is why it, and
//! not a pane, executes a resize: growing one pane must shrink its neighbour,
//! and a pane knows nothing of its neighbours. It is the lowest level that
//! contains both sides of a border — the rule from D27, in its first
//! non-obvious application.
//!
//! [`Layout`] is not a general tree. Every arrangement we know of
//! — a diff alone, explorer beside a diff, history beside a diff — is one pane
//! or two. Helix's `Tree` is around 600 lines with climb-and-descend
//! directional focus, and buys nothing until a third arrangement exists. The
//! seam is a single enum in this file, and nothing outside it knows the
//! difference.

use super::{BufferId, Pane};

/// Columns the list gets when a tab first splits.
///
/// Wide enough for a name, an indent and a status letter without wrapping,
/// which is what the plugin also settled on.
const DEFAULT_LEFT: u16 = 40;
/// Narrow enough to be nearly shut, without becoming a border with nothing
/// behind it.
const MIN_LEFT: u16 = 12;
const MAX_LEFT: u16 = 100;

/// A pane's place in [`Tab::panes`].
///
/// An index, for the same reason as [`BufferId`]: a reference here would make
/// the view self-referential. The rule throughout is that an id lives with the
/// collection it indexes.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct PaneId(usize);

/// How a tab arranges its panes.
///
/// A width in columns rather than a rectangle: where the border lands on a
/// given screen is `render::layout`'s arithmetic, and a tab that held
/// rectangles would have to be told the screen size before it could answer
/// anything about itself.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Layout {
    /// One pane, filling the tab.
    Full,
    /// A list on the left, what it opened on the right.
    Split {
        /// Columns the left-hand pane gets.
        left: u16,
    },
}

/// An independent arrangement of panes.
#[derive(Debug)]
pub struct Tab {
    panes: Vec<Pane>,
    focus: PaneId,
    layout: Layout,
}

impl Tab {
    pub fn single(buffer: BufferId) -> Self {
        Self {
            panes: vec![Pane::new(buffer)],
            focus: PaneId(0),
            layout: Layout::Full,
        }
    }

    pub fn layout(&self) -> Layout {
        self.layout
    }

    /// Puts `buffer` in the right-hand pane, splitting the tab if it is whole.
    ///
    /// Here rather than on a pane because it can create one, and a pane cannot
    /// bring a sibling into being. The focus does not move: a reader working
    /// down a list of files wants the next file, not the pane they just filled
    /// — which is what the plugin's `focus_on_select = false` also decided.
    pub fn show(&mut self, buffer: BufferId) {
        match self.layout {
            Layout::Full => {
                self.panes.push(Pane::new(buffer));
                self.layout = Layout::Split { left: DEFAULT_LEFT };
            }
            // A fresh pane, not a repointed one. A viewport describes a place
            // in *the buffer it was looking at*, so carrying it over opens the
            // new file at the old file's line and horizontal offset — which
            // showed up as a file that opened blank, scrolled off its own
            // text.
            Layout::Split { .. } => self.panes[1] = Pane::new(buffer),
        }
    }

    /// Whether anything is shown beside the list.
    pub fn is_split(&self) -> bool {
        matches!(self.layout, Layout::Split { .. })
    }

    /// The buffer already beside the list, if there is one.
    ///
    /// What lets a second open reuse the first one's slot rather than adding
    /// to a list nothing ever removes from.
    pub fn shown(&self) -> Option<BufferId> {
        match self.layout {
            Layout::Split { .. } => Some(self.panes[1].buffer),
            Layout::Full => None,
        }
    }

    /// Moves the focus to the next pane, wrapping.
    pub fn focus_prev(&mut self) {
        // Two panes: prev and next are the same. If more panes exist later,
        // this wraps backwards.
        self.focus_next();
    }

    pub fn focus_next(&mut self) {
        self.focus = PaneId((self.focus.0 + 1) % self.panes.len());
    }

    /// Focuses a specific pane, if it exists.
    pub fn set_focus(&mut self, id: PaneId) {
        if id.0 < self.panes.len() {
            self.focus = id;
        }
    }

    /// Moves the border between the panes.
    ///
    /// Executed here, not by a pane: growing one shrinks the other, and a pane
    /// knows nothing of its neighbours. This is the rule from D27 in the case
    /// that produced it.
    pub fn resize(&mut self, by: i32) {
        let Layout::Split { left } = self.layout else {
            return;
        };
        let moved = i32::from(left).saturating_add(by);
        self.layout = Layout::Split {
            left: moved.clamp(i32::from(MIN_LEFT), i32::from(MAX_LEFT)) as u16,
        };
    }

    /// Which pane has focus, as an index into [`Self::panes`].
    pub fn focus(&self) -> PaneId {
        self.focus
    }

    pub fn focused(&self) -> &Pane {
        &self.panes[self.focus.0]
    }

    pub fn focused_mut(&mut self) -> &mut Pane {
        &mut self.panes[self.focus.0]
    }

    /// Every pane, in layout order.
    pub fn panes(&self) -> &[Pane] {
        &self.panes
    }

    /// Every pane's id, in layout order.
    ///
    /// Ids rather than panes, so a caller can walk them while borrowing the
    /// view mutably to draw each one.
    pub fn ids(&self) -> impl Iterator<Item = PaneId> {
        (0..self.panes.len()).map(PaneId)
    }

    pub fn pane(&self, id: PaneId) -> &Pane {
        &self.panes[id.0]
    }

    pub fn pane_mut(&mut self, id: PaneId) -> &mut Pane {
        &mut self.panes[id.0]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn split() -> Tab {
        let mut tab = Tab::single(BufferId::new(0));
        tab.show(BufferId::new(1));
        tab
    }

    #[test]
    fn a_count_larger_than_the_screen_moves_the_border_to_its_limit() {
        // `99999>` panicked before this: the step and the count were
        // multiplied as i16, and the product overflowed long before the border
        // reached either end.
        let mut tab = split();
        for by in [i32::MAX, i32::MIN, 400_000, -400_000, 0] {
            tab.resize(by);
            let Layout::Split { left } = tab.layout() else {
                panic!("the tab stopped being split");
            };
            assert!((MIN_LEFT..=MAX_LEFT).contains(&left), "left is {left}");
        }
    }

    #[test]
    fn showing_a_second_buffer_replaces_the_pane_rather_than_repointing_it() {
        // A viewport describes a place in *the buffer it was looking at*.
        // Carrying it over opened the new file at the old file's line, which
        // showed up as a file that opened blank.
        let mut tab = split();
        tab.pane_mut(PaneId(1)).viewport.jump(120, 400);
        assert_eq!(tab.pane(PaneId(1)).viewport.cursor(), 120);

        tab.show(BufferId::new(2));
        assert_eq!(tab.pane(PaneId(1)).buffer, BufferId::new(2));
        assert_eq!(tab.pane(PaneId(1)).viewport.cursor(), 0, "at its own top");
    }

    #[test]
    fn the_slot_beside_the_list_is_named_so_a_second_open_can_reuse_it() {
        let mut tab = Tab::single(BufferId::new(0));
        assert_eq!(tab.shown(), None, "nothing is beside it yet");
        tab.show(BufferId::new(1));
        assert_eq!(tab.shown(), Some(BufferId::new(1)));
        tab.show(BufferId::new(2));
        assert_eq!(tab.shown(), Some(BufferId::new(2)));
        assert_eq!(tab.panes().len(), 2, "and no third pane appeared");
    }

    #[test]
    fn focus_moves_between_the_panes_and_wraps() {
        let mut tab = split();
        assert_eq!(tab.focus(), PaneId(0));
        tab.focus_next();
        assert_eq!(tab.focus(), PaneId(1));
        tab.focus_next();
        assert_eq!(tab.focus(), PaneId(0), "and wraps rather than running off");
    }

    #[test]
    fn a_whole_tab_has_no_border_to_move() {
        // `>` is bound in the list, and the list exists before anything is
        // opened beside it.
        let mut tab = Tab::single(BufferId::new(0));
        tab.resize(20);
        assert_eq!(tab.layout(), Layout::Full);
    }
}
