//! How the files under a heading are arranged, and asking whichever it is.
//!
//! One variant per arrangement, and the same four questions of each: how many
//! lines, what is on one, which file it stands for, and whether it opens. The
//! answers differ; the questions do not, which is what lets `draw` loop over
//! lines without knowing what it is drawing.
//!
//! An enum rather than a trait: the arrangements are a closed set, so an
//! exhaustive `match` means adding one breaks the build until it is handled
//! everywhere — the same property that stops the keymap growing dead commands,
//! and the same reason `BufferType` is one.
//!
//! **No arrangement knows a heading exists.** Each is handed one group's files
//! and nothing else, so a third one is a variant here and no change anywhere
//! above. See D69.

use file_types::File;

use super::{List, Tree, ViewLine};

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
}
