//! Turning a list of paths into something walkable.
//!
//! Three steps, in order: put every file under its directories, collapse the
//! directory chains that have nothing to choose between, then sort each
//! directory's children. Flattening runs before sorting because it changes
//! what a name is — `only/one/chain` sorts as one name, not three.

use crate::node::{EntryId, Node, NodeId, NodeType};
use crate::order;
use crate::{Entry, Groups};

/// How the files are arranged.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ViewMode {
    /// One row per file, showing its whole path.
    List,
    /// Directories as rows of their own, with their files under them.
    #[default]
    Tree,
}

/// Every node of every section, and the section roots.
#[derive(Debug, Default)]
pub struct Tree {
    nodes: Vec<Node>,
    /// One per group that has anything in it, in the order they arrived.
    ///
    /// The order is the pipeline's answer, not ours: it knows that what is
    /// unstaged is read before what is staged, and a comparison of two
    /// revisions has only one group to order.
    roots: Vec<NodeId>,
}

impl Tree {
    /// Builds the tree for one view mode.
    ///
    /// Entries are indexed by position, so the caller keeps its own list and
    /// this holds only numbers into it.
    pub fn build(groups: &Groups, mode: ViewMode, flatten: bool) -> Self {
        let mut tree = Tree::default();
        for (index, group) in groups.iter().enumerate() {
            if group.is_empty() {
                continue;
            }
            let members: Vec<EntryId> = (0..group.files.len())
                .map(|file| EntryId { group: index, file })
                .collect();
            let root = tree.push(Node::new(&group.name, NodeType::Heading(index)));
            match mode {
                ViewMode::List => tree.fill_flat(root, &members, groups),
                ViewMode::Tree => tree.fill_nested(root, &members, groups, flatten),
            }
            tree.roots.push(root);
        }
        tree
    }

    pub fn node(&self, id: NodeId) -> &Node {
        &self.nodes[id.0]
    }

    pub fn roots(&self) -> &[NodeId] {
        &self.roots
    }

    fn push(&mut self, node: Node) -> NodeId {
        self.nodes.push(node);
        NodeId(self.nodes.len() - 1)
    }

    /// One row per file, each showing its whole path, sorted as VS Code sorts.
    fn fill_flat(&mut self, root: NodeId, members: &[EntryId], groups: &Groups) {
        let mut rows: Vec<EntryId> = members.to_vec();
        rows.sort_by(|&a, &b| order::by_name(at(groups, a).path(), at(groups, b).path()));
        for id in rows {
            let child = self.push(Node::new(at(groups, id).path(), NodeType::File(id)));
            self.nodes[root.0].children.push(child);
        }
    }

    /// Directories as rows, files under them.
    fn fill_nested(&mut self, root: NodeId, members: &[EntryId], groups: &Groups, flatten: bool) {
        for &id in members {
            let path = at(groups, id).path();
            let (directories, name) = split(path);
            let mut parent = root;
            for segment in directories {
                parent = self.directory(parent, segment);
            }
            let child = self.push(Node::new(name, NodeType::File(id)));
            self.nodes[parent.0].children.push(child);
        }
        if flatten {
            self.flatten(root);
        }
        self.sort(root);
    }

    /// The child directory of `parent` called `name`, created if it is new.
    fn directory(&mut self, parent: NodeId, name: &str) -> NodeId {
        let existing = self.nodes[parent.0]
            .children
            .iter()
            .copied()
            .find(|&child| self.nodes[child.0].is_directory() && self.nodes[child.0].name == name);
        if let Some(id) = existing {
            return id;
        }
        let id = self.push(Node::new(name, NodeType::Directory));
        self.nodes[parent.0].children.push(id);
        id
    }

    /// Collapses every chain of directories that has nothing to choose
    /// between into a single row.
    ///
    /// `src/main/rust/app.rs` alone in a repository is four rows of tree and
    /// one file, and three of those rows offer the reader no decision. VS
    /// Code, GitHub and every file explorer that has thought about it do the
    /// same. A directory holding *one file* is left alone: the file is the
    /// content, not a step on the way to it.
    fn flatten(&mut self, node: NodeId) {
        while self.nodes[node.0].is_directory() && self.nodes[node.0].children.len() == 1 {
            let only = self.nodes[node.0].children[0];
            if !self.nodes[only.0].is_directory() {
                break;
            }
            let name = std::mem::take(&mut self.nodes[only.0].name);
            let children = std::mem::take(&mut self.nodes[only.0].children);
            self.nodes[node.0].name = format!("{}/{name}", self.nodes[node.0].name);
            self.nodes[node.0].children = children;
        }
        for child in self.nodes[node.0].children.clone() {
            self.flatten(child);
        }
    }

    /// Sorts every directory's children, deepest first order being irrelevant.
    fn sort(&mut self, node: NodeId) {
        let mut children = std::mem::take(&mut self.nodes[node.0].children);
        children.sort_by(|&a, &b| {
            order::in_tree(
                (self.nodes[a.0].is_directory(), &self.nodes[a.0].name),
                (self.nodes[b.0].is_directory(), &self.nodes[b.0].name),
            )
        });
        self.nodes[node.0].children = children;
        for child in self.nodes[node.0].children.clone() {
            self.sort(child);
        }
    }
}

/// The entry an id names.
pub(crate) fn at(groups: &Groups, id: EntryId) -> &Entry {
    &groups[id.group].files[id.file]
}

/// A path's directories and its file name.
fn split(path: &str) -> (Vec<&str>, &str) {
    let mut segments: Vec<&str> = path.split('/').filter(|s| !s.is_empty()).collect();
    let name = segments.pop().unwrap_or(path);
    (segments, name)
}
