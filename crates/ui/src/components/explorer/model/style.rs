//! How the files under a heading are arranged.
//!
//! One variant per arrangement: tree (with directories) or list (flat paths).
//! Each answers the same four questions: line count, what is on a line, which
//! file it is, and whether it opens.

use file_types::File;

use super::{List, NodeId, Tree, ViewLine};

/// How the files under a heading are arranged.
#[derive(Debug)]
pub enum Style {
    /// Directories as lines of their own, with their files under them.
    Tree(Tree),
    /// One line per file, showing its whole path.
    List(List),
}

impl Style {
    /// How many lines this arrangement takes.
    pub fn view_lines(&self) -> u32 {
        match self {
            Style::Tree(tree) => tree.view_lines().len() as u32,
            Style::List(list) => list.view_lines().len() as u32,
        }
    }

    /// The file on a line, as a place in the list the explorer holds.
    pub fn file_on(&self, line: usize) -> Option<usize> {
        match self {
            Style::Tree(tree) => tree.file_on(line),
            Style::List(list) => list.file_on(line),
        }
    }

    /// What is on a line, as facts.
    pub fn view_line<'a>(&'a self, line: usize, files: &'a [File]) -> Option<ViewLine<'a>> {
        match self {
            Style::Tree(tree) => tree.view_line(line, files),
            Style::List(list) => list.view_line(line, files),
        }
    }

    /// Opens or shuts what is on a line, and says whether it did.
    pub fn toggle(&mut self, line: usize) -> bool {
        match self {
            Style::Tree(tree) => tree.toggle(line),
            // Nothing in a flat list has anything under it. Its heading folds,
            // and that is the group's, one level up.
            Style::List(_) => false,
        }
    }

    /// The node on a line that can be opened and shut, if there is one.
    pub fn foldable_on(&self, line: usize) -> Option<NodeId> {
        match self {
            Style::Tree(tree) => tree.foldable_on(line),
            Style::List(_) => None,
        }
    }

    /// Which nodes are shut, which for a flat list is none.
    pub fn closed(&self) -> Vec<NodeId> {
        match self {
            Style::Tree(tree) => tree.closed(),
            Style::List(_) => Vec::new(),
        }
    }

    /// Shuts exactly these nodes and opens every other.
    pub fn set_closed(&mut self, closed: &[NodeId]) {
        if let Style::Tree(tree) = self {
            tree.set_closed(closed);
        }
    }
}
