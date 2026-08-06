//! The explorer's state, and everything that changes it.
//!
//! Rows are rebuilt rather than patched. A fold changes how many rows exist
//! and what every guide below it looks like, so patching would mean writing
//! the walk twice — once forwards and once as a correction — and the two would
//! eventually disagree. The walk is cheap: it is a few thousand rows at worst,
//! and it runs on a keypress, not on a frame.

use std::collections::BTreeSet;

use crate::node::{Node, NodeId};
use crate::rows::{self, Settings};
use crate::tree::at;
use crate::tree::{Tree, ViewMode};
use crate::{Entry, Groups, Row};

/// One file, named so it can be found again after the rows are rebuilt.
///
/// The comparison as well as the path, because a file staged and then edited
/// again is listed twice and the two rows are different diffs. `Revs` rather
/// than a group number: a number means nothing once the groups are rebuilt.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Anchor {
    pub revs: file_types::Revs,
    pub path: String,
}

/// The list of changed files, as the reader is looking at it.
#[derive(Debug)]
pub struct Explorer {
    groups: Groups,
    mode: ViewMode,
    /// Whether chains of single-child directories collapse into one row.
    flatten: bool,
    stats_shown: bool,
    /// Nodes that are shut. Absent means open, so a freshly built tree is
    /// fully open without anything having to enumerate it.
    collapsed: BTreeSet<NodeId>,
    /// What is hiding rows, as the reader typed it.
    pattern: Option<String>,
    /// The groups the tree was built from — everything, or what a pattern
    /// left. Kept apart from [`Self::groups`] because a node holds an index
    /// into this list, and clearing a pattern must not change what those
    /// indices mean without rebuilding.
    shown: Groups,
    tree: Tree,
    rows: Vec<Row>,
    /// Which row the reader is on, as an index into [`Self::rows`].
    selected: usize,
}

impl Explorer {
    /// Builds the explorer over a set of groups, everything open.
    pub fn new(groups: Groups) -> Self {
        let mut explorer = Self {
            groups,
            mode: ViewMode::default(),
            flatten: true,
            stats_shown: true,
            collapsed: BTreeSet::new(),
            pattern: None,
            shown: Groups::new(),
            tree: Tree::default(),
            rows: Vec::new(),
            selected: 0,
        };
        explorer.reshape();
        explorer.selected = explorer.first_file().unwrap_or(0);
        explorer
    }

    pub fn rows(&self) -> &[Row] {
        &self.rows
    }

    pub fn is_empty(&self) -> bool {
        self.rows.is_empty()
    }

    pub fn selected(&self) -> usize {
        self.selected
    }

    /// What the reader is looking at, in terms that survive a rebuild.
    ///
    /// A row number does not: changing the view mode or a filter renumbers
    /// every row, and node ids do not survive either. A comparison and a path
    /// do, because they are what the reader chose.
    pub fn anchor(&self, row: usize) -> Option<Anchor> {
        let id = self.tree.node(self.rows.get(row)?.node).entry()?;
        Some(Anchor {
            revs: self.shown[id.group].revs.clone(),
            path: at(&self.shown, id).path().to_owned(),
        })
    }

    /// Where that file is now, if it is still listed.
    pub fn row_of(&self, anchor: &Anchor) -> Option<usize> {
        self.rows.iter().position(|row| {
            self.tree.node(row.node).entry().is_some_and(|id| {
                self.shown[id.group].revs == anchor.revs
                    && at(&self.shown, id).path() == anchor.path
            })
        })
    }

    /// The first row that is a file, if there is one.
    ///
    /// Where a reader starts. Row zero is a heading, which can be folded but
    /// not opened, so starting there would mean the first key press did
    /// nothing — and the plugin starts on the first file for the same reason.
    pub fn first_file(&self) -> Option<usize> {
        self.rows
            .iter()
            .position(|row| self.tree.node(row.node).entry().is_some())
    }

    pub fn mode(&self) -> ViewMode {
        self.mode
    }

    /// Moves the selection to a row, if there is one there.
    pub fn select(&mut self, row: usize) {
        if row < self.rows.len() {
            self.selected = row;
        }
    }

    /// The node the reader is on.
    pub fn node(&self) -> Option<&Node> {
        let row = self.rows.get(self.selected)?;
        Some(self.tree.node(row.node))
    }

    /// The file the reader is on, or `None` on a heading or a directory.
    pub fn entry(&self) -> Option<&Entry> {
        Some(at(&self.shown, self.node()?.entry()?))
    }

    /// The file a given row stands for, without moving the selection.
    pub fn entry_of(&self, row: &Row) -> Option<&Entry> {
        Some(at(&self.shown, self.tree.node(row.node).entry()?))
    }

    /// Whether the selected row can be opened and shut.
    pub fn is_foldable(&self) -> bool {
        self.node().is_some_and(Node::is_foldable)
    }

    /// Opens the selected row if it is shut, shuts it if it is open.
    ///
    /// Returns whether anything happened, so a key bound to both this and
    /// opening a file can tell which it did.
    pub fn toggle(&mut self) -> bool {
        let Some(row) = self.rows.get(self.selected) else {
            return false;
        };
        let id = row.node;
        if !self.tree.node(id).is_foldable() {
            return false;
        }
        if !self.collapsed.insert(id) {
            self.collapsed.remove(&id);
        }
        self.rebuild();
        true
    }

    pub fn set_mode(&mut self, mode: ViewMode) {
        self.mode = mode;
        self.reshape();
    }

    pub fn toggle_mode(&mut self) {
        self.set_mode(match self.mode {
            ViewMode::Tree => ViewMode::List,
            ViewMode::List => ViewMode::Tree,
        });
    }

    pub fn set_flatten(&mut self, flatten: bool) {
        self.flatten = flatten;
        self.reshape();
    }

    pub fn set_stats(&mut self, shown: bool) {
        self.stats_shown = shown;
        self.rebuild();
    }

    pub fn toggle_stats(&mut self) {
        self.set_stats(!self.stats_shown);
    }

    /// Hides every file whose path does not match a glob.
    ///
    /// `None` shows everything again.
    pub fn set_pattern(&mut self, pattern: Option<String>) {
        self.pattern = pattern;
        self.reshape();
    }

    pub fn pattern(&self) -> Option<&str> {
        self.pattern.as_deref()
    }

    /// Replaces the files, keeping how the reader had arranged the view.
    ///
    /// For a refresh: the mode, the pattern and the stats switch survive
    /// because they are the reader's choices, while the folds do not, because
    /// a node id means nothing once the tree has been rebuilt from different
    /// files.
    pub fn refresh(&mut self, groups: Groups) {
        self.groups = groups;
        self.reshape();
    }

    /// Rebuilds the tree as well as the rows.
    ///
    /// Anything that changes which nodes exist has to come through here,
    /// because a fold is a node id and the ids do not survive.
    fn reshape(&mut self) {
        self.collapsed.clear();
        self.rebuild_tree();
        self.rebuild();
    }

    fn rebuild_tree(&mut self) {
        // A pattern narrows every group and removes none: a group that a
        // filter empties is skipped by the tree, so its heading goes without
        // the group numbers shifting under the nodes that hold them.
        let kept: Groups = match &self.pattern {
            None => self.groups.clone(),
            Some(pattern) => self
                .groups
                .iter()
                .map(|group| {
                    let files = group
                        .files
                        .iter()
                        .filter(|entry| crate::filter::matches(pattern, entry.path()))
                        .cloned()
                        .collect();
                    crate::Group::new(&group.name, group.revs.clone(), files)
                })
                .collect(),
        };
        self.tree = Tree::build(&kept, self.mode, self.flatten);
        self.shown = kept;
    }

    fn rebuild(&mut self) {
        self.rows = rows::walk(
            &self.tree,
            &self.shown,
            &self.collapsed,
            &Settings {
                stats_shown: self.stats_shown,
            },
        );
        // A fold removes rows under the selection, so it can leave the reader
        // past the end. Landing on the last row is what every list does, and
        // is closer to where they were than row zero would be.
        self.selected = self.selected.min(self.rows.len().saturating_sub(1));
    }
}
