//! Walking the tree into the lines that are actually on screen.
//!
//! One pass, depth first, skipping the children of anything shut. The indent
//! guides are built here rather than stored on a node because they are not a
//! property of the node: `│` at a given depth means *an ancestor at that depth
//! has more children after it*, which is a fact about the walk.

use std::collections::BTreeSet;

use file_types::Stats;

use crate::Entry;
use crate::Groups;
use crate::node::{Node, NodeId, NodeType};
use crate::row::{Region, RegionType, Row, priority};
use crate::tree::Tree;
use crate::tree::at;

/// How a directory says whether it is open.
///
/// Triangles rather than nerd-font folders, so the explorer is readable in a
/// terminal with any font. One place to change if that is ever configurable.
const OPEN: &str = "▾ ";
const SHUT: &str = "▸ ";

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
    let node = tree.node(root);
    let files = count(tree, root);
    let mut left = vec![Region::fixed(&node.name, RegionType::Heading)];

    let total: Stats = totals(tree, groups, root);
    if settings.stats_shown && !total.is_empty() {
        left.push(Region::droppable(
            format!(" ({files} · "),
            RegionType::Count,
            priority::COUNT,
        ));
        push_stats(&mut left, total, priority::COUNT);
        left.push(Region::droppable(")", RegionType::Count, priority::COUNT));
    } else {
        left.push(Region::droppable(
            format!(" ({files})"),
            RegionType::Count,
            priority::COUNT,
        ));
    }

    Row {
        node: root,
        left,
        right: Vec::new(),
    }
}

/// Emits every child of `parent`, and their children in turn.
///
/// `ancestors` holds, for each level between the section and this one, whether
/// that ancestor was the last of its siblings — which is exactly the question
/// "does this column need a `│`".
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
        let node = tree.node(child);
        rows.push(match node.node_type {
            NodeType::File(id) => file(child, node, at(groups, id), ancestors, is_last, settings),
            _ => directory(child, node, ancestors, is_last, collapsed),
        });
        if node.is_foldable() && !collapsed.contains(&child) {
            ancestors.push(is_last);
            descend(tree, groups, collapsed, settings, child, ancestors, rows);
            ancestors.pop();
        }
    }
}

fn directory(
    id: NodeId,
    node: &Node,
    ancestors: &[bool],
    is_last: bool,
    collapsed: &BTreeSet<NodeId>,
) -> Row {
    let fold = if collapsed.contains(&id) { SHUT } else { OPEN };
    Row {
        node: id,
        left: vec![
            Region::droppable(
                markers(ancestors, is_last),
                RegionType::Marker,
                priority::MARKER,
            ),
            Region::fixed(fold, RegionType::Fold),
            Region::fixed(&node.name, RegionType::Directory),
        ],
        right: Vec::new(),
    }
}

fn file(
    id: NodeId,
    node: &Node,
    entry: &Entry,
    ancestors: &[bool],
    is_last: bool,
    settings: &Settings,
) -> Row {
    let mut left = vec![
        Region::droppable(
            markers(ancestors, is_last),
            RegionType::Marker,
            priority::MARKER,
        ),
        Region::fixed(&node.name, RegionType::Name),
    ];
    if let Some(previous) = entry.moved_from() {
        left.push(Region::droppable(
            format!(" ← {previous}"),
            RegionType::Moved,
            priority::MOVED,
        ));
    }

    let mut right = Vec::new();
    let counted = entry.stats.filter(|stats| !stats.is_empty());
    if let (true, Some(stats)) = (settings.stats_shown, counted) {
        push_stats(&mut right, stats, priority::STATS);
        right.push(Region::droppable(" ", RegionType::Spacer, priority::STATS));
    }
    right.push(Region::fixed(
        entry.status(),
        RegionType::Status(entry.file.change()),
    ));

    Row {
        node: id,
        left,
        right,
    }
}

/// The `+4 -3` pair, with a side left out when it is zero.
///
/// A file that only gained lines says `+4`, not `+4 -0`: the zero is noise in
/// a column the eye is scanning.
fn push_stats(regions: &mut Vec<Region>, stats: Stats, priority: u8) {
    if stats.added > 0 {
        regions.push(Region::droppable(
            format!("+{}", stats.added),
            RegionType::Added,
            priority,
        ));
    }
    if stats.removed > 0 {
        let separator = if stats.added > 0 { " " } else { "" };
        regions.push(Region::droppable(
            format!("{separator}-{}", stats.removed),
            RegionType::Removed,
            priority,
        ));
    }
}

/// The indent guides for a row.
fn markers(ancestors: &[bool], is_last: bool) -> String {
    let mut out = String::new();
    for &ancestor_was_last in ancestors {
        out.push_str(if ancestor_was_last { "  " } else { "│ " });
    }
    out.push_str(if is_last { "└ " } else { "├ " });
    out
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_guide_is_drawn_only_where_an_ancestor_has_more_to_come() {
        // The three cases, in the order they appear down a tree.
        assert_eq!(markers(&[], false), "├ ");
        assert_eq!(markers(&[], true), "└ ");
        assert_eq!(markers(&[false], true), "│ └ ");
        // An ancestor that was last leaves blank space, not a trailing guide
        // running down beside nothing.
        assert_eq!(markers(&[true], false), "  ├ ");
        assert_eq!(markers(&[false, true], false), "│   ├ ");
    }
}
