//! The nested arrangement: directories, and the files inside them.
//!
//! ---
//!
//! **One group's files, and nothing about groups.** What a heading says, how
//! many files are under it and whether it is open are [`Explorer`]'s, because
//! a heading is what an arrangement sits *under* rather than part of one. This
//! is handed some files and nests them; a second arrangement is handed the
//! same files and does something else with them. See D69.
//!
//! **A visible line is a [`NodeId`].** Where a line sits is read off the node
//! — what it hangs from, and whether it is the last of its siblings — because
//! folding changes which nodes are *shown* and never which children a node
//! has. Both are recorded once by [`Tree::place`], after the children are in
//! their final order.
//!
//! **Facts, not characters.** That `▾` and `│ ` are how those look is `draw`'s
//! answer, beside the theme that colours them — the same division `align`
//! keeps when it reports that a view line is a gap without saying a gap is
//! drawn `╱`. See D65.
//!
//! [`Explorer`]: super::Explorer

use file_types::File;

use super::ViewLine;
use super::order;

/// A node's place in [`Tree::nodes`].
///
/// Every node of the tree lives in that one list, and a node names another by
/// its position in it — `NodeId(4)` is the node in slot four. So a directory's
/// `children` holds numbers, not nodes, and reading a child is a lookup.
///
/// A node cannot simply own its children, because then it could not also name
/// its parent: that is a loop, and Rust needs reference counting for one.
/// Counting them would make every read a borrow checked while the program
/// runs. `BufferId` and `PaneId` are numbers for the same reason.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct NodeId(usize);

/// What only one kind of node has.
///
/// The shared half is [`Node`] and this is the rest, which is how gitui's
/// `FileTreeItem` and broot's `TreeLine` — the two closest programs to this
/// one — both model the same tree. A directory's children and whether it is
/// open mean nothing for a file; a file's place in the explorer's list means
/// nothing for a directory. Neither can be given the other's, because there is
/// nowhere to put it.
#[derive(Debug, Clone)]
pub enum NodeType {
    /// One changed file, by its place in the list the explorer holds.
    File { index: usize },
    /// A directory, holding files and other directories.
    Folder {
        children: Vec<NodeId>,
        /// Whether what is inside is showing. Open is the default, so a
        /// freshly built tree needs nothing enumerated to be fully open.
        open: bool,
    },
}

/// One line of the tree, before anything decides whether it is visible.
///
/// What every node has, and then what only its own kind has. A name, what it
/// hangs from and whether it is the last of its siblings are asked of every
/// node while drawing, so they are read without asking which kind it is.
#[derive(Debug, Clone)]
pub struct Node {
    /// What is written on the line.
    ///
    /// One path segment normally, and several joined by `/` when a chain of
    /// single-child directories has been collapsed into one line.
    pub name: String,
    /// What this hangs from, or `None` at the top level. Held so the indent
    /// can be read off the tree rather than carried beside it.
    pub parent: Option<NodeId>,
    /// Whether this is the last of its siblings.
    ///
    /// Fixed once [`Tree::sort`] has run: folding changes which nodes are
    /// *shown*, never which children a node has. That is what makes the whole
    /// indent a property of the tree rather than of the walk that emits it.
    pub is_last: bool,
    pub node_type: NodeType,
}

impl Node {
    /// A directory, which holds things and stands for no file.
    fn directory(name: impl Into<String>) -> Self {
        Self::new(
            name,
            NodeType::Folder {
                children: Vec::new(),
                open: true,
            },
        )
    }

    /// A file, which stands for one and holds nothing.
    fn file(name: impl Into<String>, index: usize) -> Self {
        Self::new(name, NodeType::File { index })
    }

    fn new(name: impl Into<String>, node_type: NodeType) -> Self {
        Self {
            name: name.into(),
            parent: None,
            is_last: true,
            node_type,
        }
    }

    /// The file this stands for, or `None` for a directory.
    pub fn file_index(&self) -> Option<usize> {
        match self.node_type {
            NodeType::File { index } => Some(index),
            NodeType::Folder { .. } => None,
        }
    }

    /// What is inside, which for a file is nothing.
    pub fn children(&self) -> &[NodeId] {
        match &self.node_type {
            NodeType::Folder { children, .. } => children,
            NodeType::File { .. } => &[],
        }
    }

    /// Whether what is inside is showing. A file has nothing inside, and
    /// nothing to open.
    pub fn is_open(&self) -> bool {
        matches!(self.node_type, NodeType::Folder { open: true, .. })
    }

    /// Whether this node can be opened and shut.
    ///
    /// A directory with nothing in it cannot: there would be nothing to
    /// reveal, and a fold marker beside it would be a lie. A file never can,
    /// because there is nowhere to put anything under one.
    pub fn is_foldable(&self) -> bool {
        !self.children().is_empty()
    }

    pub fn is_directory(&self) -> bool {
        matches!(self.node_type, NodeType::Folder { .. })
    }

    /// What is inside, to be added to or reordered.
    fn children_mut(&mut self) -> Option<&mut Vec<NodeId>> {
        match &mut self.node_type {
            NodeType::Folder { children, .. } => Some(children),
            NodeType::File { .. } => None,
        }
    }
}

/// One group's files, nested by their directories.
#[derive(Debug, Default)]
pub struct Tree {
    nodes: Vec<Node>,
    /// The top level: what is directly under the heading.
    roots: Vec<NodeId>,
    /// Which node is on each line.
    ///
    /// A lookup from line number to node, and a `Vec` because the keys are
    /// `0, 1, 2 …` with no gaps — the index *is* the key. Three things need
    /// that direction and none needs the other: the viewport clamps against
    /// the length, the cursor is a line number, and drawing takes a slice.
    ///
    /// Not a property of a node. Folding one directory moves every line below
    /// it, so this is the tree *plus* what is currently shut — which is why it
    /// is rebuilt on a fold and nowhere else.
    view_lines: Vec<NodeId>,
}

impl Tree {
    /// Nests `members`, which are places in `files`.
    ///
    /// Chains of directories with nothing to choose between are collapsed —
    /// see [`Tree::collapse_chains`].
    pub fn build(files: &[File], members: &[usize]) -> Self {
        let mut tree = Tree::default();
        for &index in members {
            let (directories, name) = split(files[index].path().as_str());
            let directories: Vec<String> = directories.iter().map(|s| s.to_string()).collect();
            let name = name.to_owned();
            let mut parent = None;
            for segment in directories {
                parent = Some(tree.directory(parent, &segment));
            }
            let child = tree.push(Node::file(name, index));
            tree.children_of(parent).push(child);
        }
        tree.collapse_chains(None);
        tree.sort(None);
        tree.place(None);
        tree.reflow();
        tree
    }

    /// Which node is on each line, in order.
    pub fn view_lines(&self) -> &[NodeId] {
        &self.view_lines
    }

    pub fn node(&self, id: NodeId) -> &Node {
        &self.nodes[id.0]
    }

    /// The file on a line, as a place in the explorer's list.
    pub fn file_on(&self, line: usize) -> Option<usize> {
        self.node(*self.view_lines.get(line)?).file_index()
    }

    /// What is on a line, as facts.
    pub fn view_line<'a>(&'a self, line: usize, files: &'a [File]) -> Option<ViewLine<'a>> {
        let node = self.node(*self.view_lines.get(line)?);
        Some(match node.node_type {
            NodeType::Folder { open, .. } => ViewLine::Directory {
                name: &node.name,
                open,
            },
            NodeType::File { index } => ViewLine::File {
                name: &node.name,
                file: files.get(index)?,
            },
        })
    }

    /// Opens the node on a line if it is shut, shuts it if it is open.
    ///
    /// Returns whether anything happened, so a key bound to both this and
    /// opening a file can tell which it did.
    pub fn toggle(&mut self, line: usize) -> bool {
        let Some(&id) = self.view_lines.get(line) else {
            return false;
        };
        if !self.nodes[id.0].is_foldable() {
            return false;
        }
        let NodeType::Folder { open, .. } = &mut self.nodes[id.0].node_type else {
            return false;
        };
        *open = !*open;
        self.reflow();
        true
    }

    fn push(&mut self, node: Node) -> NodeId {
        self.nodes.push(node);
        NodeId(self.nodes.len() - 1)
    }

    /// The children of a node, or the top level when there is no node.
    ///
    /// # Panics
    ///
    /// If asked for a file's children. Only a directory is ever made a parent,
    /// which [`Tree::directory`] is what enforces.
    fn children_of(&mut self, parent: Option<NodeId>) -> &mut Vec<NodeId> {
        match parent {
            Some(id) => self.nodes[id.0]
                .children_mut()
                .expect("a file is never made a parent"),
            None => &mut self.roots,
        }
    }

    fn children(&self, parent: Option<NodeId>) -> &[NodeId] {
        match parent {
            Some(id) => self.nodes[id.0].children(),
            None => &self.roots,
        }
    }

    /// The child directory of `parent` called `name`, created if it is new.
    fn directory(&mut self, parent: Option<NodeId>, name: &str) -> NodeId {
        let existing =
            self.children(parent).iter().copied().find(|&child| {
                self.nodes[child.0].is_directory() && self.nodes[child.0].name == name
            });
        if let Some(id) = existing {
            return id;
        }
        let id = self.push(Node::directory(name));
        self.children_of(parent).push(id);
        id
    }

    /// Collapses every chain of directories that has nothing to choose
    /// between into a single line.
    ///
    /// `src/main/rust/app.rs` alone in a repository is four lines of tree and
    /// one file, and three of those lines offer the reader no decision. VS
    /// Code, GitHub and every file explorer that has thought about it do the
    /// same. A directory holding *one file* is left alone: the file is the
    /// content, not a step on the way to it.
    fn collapse_chains(&mut self, node: Option<NodeId>) {
        if let Some(id) = node {
            while self.nodes[id.0].children().len() == 1 {
                let only = self.nodes[id.0].children()[0];
                if !self.nodes[only.0].is_directory() {
                    break;
                }
                let name = std::mem::take(&mut self.nodes[only.0].name);
                let moved = std::mem::take(
                    self.nodes[only.0]
                        .children_mut()
                        .expect("a directory, just checked"),
                );
                self.nodes[id.0].name = format!("{}/{name}", self.nodes[id.0].name);
                *self.children_of(Some(id)) = moved;
            }
        }
        for child in self.children(node).to_vec() {
            self.collapse_chains(Some(child));
        }
    }

    /// Sorts every directory's children, the order it descends in being
    /// irrelevant.
    ///
    /// A key per child, built once, then a plain sort — see
    /// [`order`](super::order) for why that is not the same as a comparator
    /// that folds case. The name is carried beside the key because the key is
    /// deliberately not a total order: two spellings of one name fold to one
    /// key, and lines that swapped between refreshes would move under the
    /// reader.
    fn sort(&mut self, node: Option<NodeId>) {
        let children = self.children(node).to_vec();
        if children.is_empty() {
            return;
        }
        let mut keyed: Vec<(Vec<u8>, &str, NodeId)> = children
            .iter()
            .map(|&child| {
                let node = &self.nodes[child.0];
                (
                    order::tree_key(node.is_directory(), &node.name),
                    node.name.as_str(),
                    child,
                )
            })
            .collect();
        keyed.sort_by(|left, right| left.0.cmp(&right.0).then_with(|| left.1.cmp(right.1)));
        let sorted: Vec<NodeId> = keyed.into_iter().map(|(_, _, child)| child).collect();
        *self.children_of(node) = sorted;

        for child in self.children(node).to_vec() {
            self.sort(Some(child));
        }
    }

    /// Records where every node hangs from, and whether it is the last of its
    /// siblings.
    ///
    /// Run once the children are in their final order, which is what makes
    /// both answers permanent — and so readable from the node instead of
    /// carried beside it.
    fn place(&mut self, node: Option<NodeId>) {
        let children = self.children(node).to_vec();
        let last = children.len();
        for (index, child) in children.into_iter().enumerate() {
            self.nodes[child.0].parent = node;
            self.nodes[child.0].is_last = index + 1 == last;
            self.place(Some(child));
        }
    }

    /// Rebuilds the line lookup.
    ///
    /// One pass, depth first, skipping the children of anything shut.
    fn reflow(&mut self) {
        let mut lines = Vec::new();
        self.descend(None, &mut lines);
        self.view_lines = lines;
    }

    fn descend(&self, parent: Option<NodeId>, lines: &mut Vec<NodeId>) {
        for &child in self.children(parent) {
            lines.push(child);
            if self.nodes[child.0].is_foldable() && self.nodes[child.0].is_open() {
                self.descend(Some(child), lines);
            }
        }
    }
}

/// A path's directories and its file name.
fn split(path: &str) -> (Vec<&str>, &str) {
    let mut segments: Vec<&str> = path.split('/').filter(|s| !s.is_empty()).collect();
    let name = segments.pop().unwrap_or(path);
    (segments, name)
}

#[cfg(test)]
mod tests {
    use super::*;
    use file_types::{File, Oid, RepoPath, Revs};
    use std::path::Path;

    fn files(paths: &[&str]) -> Vec<File> {
        paths
            .iter()
            .map(|path| {
                File::unchanged_path(
                    RepoPath::new(*path, Path::new("/repo")),
                    Revs::worktree_against(Oid::new("b87b24c")),
                )
            })
            .collect()
    }

    fn built(paths: &[&str]) -> (Vec<File>, Tree) {
        let files = files(paths);
        let members: Vec<usize> = (0..files.len()).collect();
        let tree = Tree::build(&files, &members);
        (files, tree)
    }

    #[test]
    fn an_empty_directory_cannot_be_folded() {
        // Not a hypothetical: a filter can empty a directory that had files
        // in it, and a fold marker beside nothing reads as a broken line.
        let empty = Node::directory("src");
        assert!(!empty.is_foldable());

        // A file has nowhere to put children at all, which is what makes the
        // two kinds different types rather than one with an empty list.
        let file = Node::file("a.rs", 0);
        assert!(!file.is_foldable());
        assert!(file.children().is_empty());
        assert!(!file.is_open(), "and nothing to open");

        let mut full = Node::directory("src");
        full.children_mut().expect("a directory").push(NodeId(1));
        assert!(full.is_foldable());
    }

    #[test]
    fn the_top_level_hangs_from_nothing() {
        // There is no heading node here: a heading is what a tree sits under,
        // and it belongs to whatever holds the groups.
        let (_, tree) = built(&["a.rs", "src/b.rs"]);
        for &root in &tree.roots {
            assert_eq!(tree.node(root).parent, None);
        }
    }

    #[test]
    fn shutting_a_node_removes_the_lines_under_it() {
        let (_, mut tree) = built(&["src/a.rs", "src/b.rs"]);
        let before = tree.view_lines().len();
        assert!(tree.toggle(0), "line 0 is the `src` directory");
        assert_eq!(tree.view_lines().len(), before - 2);
        assert!(tree.toggle(0));
        assert_eq!(tree.view_lines().len(), before);
    }

    #[test]
    fn a_chain_with_no_choice_in_it_becomes_one_line() {
        let (_, tree) = built(&["deep/only/one/chain/leaf.txt"]);
        assert_eq!(tree.node(tree.roots[0]).name, "deep/only/one/chain");
        assert_eq!(tree.view_lines().len(), 2, "the chain, and the file");
    }
}
