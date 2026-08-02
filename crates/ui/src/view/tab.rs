//! An independent arrangement of panes.
//!
//! A tab owns its panes and decides where each one goes. That is why it, and
//! not a pane, executes a resize: growing one pane must shrink its neighbour,
//! and a pane knows nothing of its neighbours. It is the lowest level that
//! contains both sides of a border — the rule from D27, in its first
//! non-obvious application.
//!
//! [`Layout`] is deliberately not a general tree. Every arrangement we know of
//! — a diff alone, explorer beside a diff, history beside a diff — is one pane
//! or two. Helix's `Tree` is around 600 lines with climb-and-descend
//! directional focus, and buys nothing until a third arrangement exists. The
//! seam is a single enum in this file, and nothing outside it knows the
//! difference.

use super::{BufferId, Pane};

/// A pane's place in [`Tab::panes`].
///
/// An index, for the same reason as [`BufferId`]: a reference here would make
/// the view self-referential. The rule throughout is that an id lives with the
/// collection it indexes.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct PaneId(usize);

/// How a tab arranges its panes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Layout {
    /// One pane, filling the tab.
    Full,
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
}
