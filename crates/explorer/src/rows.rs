//! Walking the tree into the lines that are actually on screen.
//!
//! One pass, depth first, skipping the children of anything shut. Where a row
//! sits is recorded rather than drawn: a guide at a given depth means *an
//! ancestor at that depth has more children after it*, which is a fact about
//! the walk and not a property of the node. What that fact looks like is
//! `ui`'s answer.

use std::collections::BTreeSet;

use file_types::Stats;

use crate::Entry;
use crate::Groups;
use crate::node::{Node, NodeId, NodeType};
use crate::row::{Content, Guides, Row};
use crate::tree::Tree;
use crate::tree::at;

/// What the walk needs to know beyond the tree itself.
pub struct Settings {
    pub stats_shown: bool,
}

/// Every row that is visible, in order.
pub fn walk(
    tree: &Tree,
    groups: &Groups,
    collapsed: &BTreeSet<NodeId>,
    settings: &Settings,
) -> Vec<Row> {
    let mut rows = Vec::new();
    for &root in tree.roots() {
        rows.push(heading(tree, groups, root, settings));
        if !collapsed.contains(&root) {
            descend(
                tree,
                groups,
                collapsed,
                settings,
                root,
                &mut Vec::new(),
                &mut rows,
            );
        }
    }
    rows
}

/// A section's heading: its title, how many files it holds, and their total.
fn heading(tree: &Tree, groups: &Groups, root: NodeId, settings: &Settings) -> Row {
    let total = totals(tree, groups, root);
    Row {
        node: root,
        guides: None,
        content: Content::Heading {
            title: tree.node(root).name.clone(),
            files: count(tree, root),
            stats: (settings.stats_shown && !total.is_empty()).then_some(total),
        },
    }
}

/// Emits every child of `parent`, and their children in turn.
///
/// `ancestors` holds, for each level between the section and this one, whether
/// that ancestor was the last of its siblings.
fn descend(
    tree: &Tree,
    groups: &Groups,
    collapsed: &BTreeSet<NodeId>,
    settings: &Settings,
    parent: NodeId,
    ancestors: &mut Vec<bool>,
    rows: &mut Vec<Row>,
) {
    let children = &tree.node(parent).children;
    for (index, &child) in children.iter().enumerate() {
        let is_last = index + 1 == children.len();
        let guides = Guides {
            ancestors: ancestors.clone(),
            is_last,
        };
        let node = tree.node(child);
        rows.push(match node.node_type {
            NodeType::File(id) => file(child, node, at(groups, id), guides, settings),
            _ => directory(child, node, guides, collapsed),
        });
        if node.is_foldable() && !collapsed.contains(&child) {
            ancestors.push(is_last);
            descend(tree, groups, collapsed, settings, child, ancestors, rows);
            ancestors.pop();
        }
    }
}

fn directory(id: NodeId, node: &Node, guides: Guides, collapsed: &BTreeSet<NodeId>) -> Row {
    Row {
        node: id,
        guides: Some(guides),
        content: Content::Directory {
            name: node.name.clone(),
            open: !collapsed.contains(&id),
        },
    }
}

fn file(id: NodeId, node: &Node, entry: &Entry, guides: Guides, settings: &Settings) -> Row {
    // A file that gained and lost nothing says nothing, rather than `+0 -0` in
    // a column the eye is scanning.
    let stats = entry
        .stats
        .filter(|stats| settings.stats_shown && !stats.is_empty());
    Row {
        node: id,
        guides: Some(guides),
        content: Content::File {
            name: node.name.clone(),
            moved_from: entry.moved_from().map(str::to_owned),
            stats,
            change: entry.file.change(),
        },
    }
}

/// How many files are under a node, however deeply.
fn count(tree: &Tree, node: NodeId) -> usize {
    match tree.node(node).node_type {
        NodeType::File(_) => 1,
        _ => tree
            .node(node)
            .children
            .iter()
            .map(|&child| count(tree, child))
            .sum(),
    }
}

/// The lines gained and lost under a node, summed.
fn totals(tree: &Tree, groups: &Groups, node: NodeId) -> Stats {
    match tree.node(node).node_type {
        NodeType::File(id) => at(groups, id).stats.unwrap_or_default(),
        _ => tree
            .node(node)
            .children
            .iter()
            .map(|&child| totals(tree, groups, child))
            .sum(),
    }
}
