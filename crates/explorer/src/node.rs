//! What the tree is made of.
//!
//! One arena of nodes with index references, for the same reason `BufferId`
//! and `PaneId` are indices: a tree of owning pointers cannot be moved, and a
//! tree of `Rc<RefCell<_>>` makes every read a runtime borrow. The whole
//! explorer is one arena, so a fold is one number in one set whatever section
//! it is in.
//!
//! A section header is a node like any other. That is deliberate: folding a
//! section and folding a directory are then the same operation, walked by the
//! same function, and neither can drift from the other.

/// A node's place in [`Tree::nodes`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct NodeId(pub(crate) usize);

/// An entry's place: which group it is in, and where in that group.
///
/// A pair rather than one number, because the groups are what the caller hands
/// in and flattening them here would mean holding the same files twice.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct EntryId {
    pub(crate) group: usize,
    pub(crate) file: usize,
}

/// What a node stands for.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NodeType {
    /// A heading, holding everything in one comparison.
    ///
    /// The number is which group, so the row can be told what to say without
    /// the node holding a copy of the name.
    Heading(usize),
    /// A directory, holding files and other directories.
    Directory,
    /// One changed file.
    File(EntryId),
}

/// One line of the tree, before anything decides whether it is visible.
#[derive(Debug, Clone)]
pub struct Node {
    /// What is written on the row.
    ///
    /// One path segment normally, and several joined by `/` when a chain of
    /// single-child directories has been collapsed into one row.
    pub name: String,
    pub node_type: NodeType,
    /// Empty for a file, which is what makes a file unfoldable.
    pub children: Vec<NodeId>,
}

impl Node {
    pub(crate) fn new(name: impl Into<String>, node_type: NodeType) -> Self {
        Self {
            name: name.into(),
            node_type,
            children: Vec::new(),
        }
    }

    /// Whether this node can be opened and closed.
    ///
    /// A directory with nothing in it cannot: there would be nothing to
    /// reveal, and a fold marker beside it would be a lie.
    pub fn is_foldable(&self) -> bool {
        !self.children.is_empty()
    }

    /// The entry this row stands for, if it stands for a file.
    pub fn entry(&self) -> Option<EntryId> {
        match self.node_type {
            NodeType::File(id) => Some(id),
            _ => None,
        }
    }

    pub fn is_directory(&self) -> bool {
        matches!(self.node_type, NodeType::Directory)
    }

    /// Which group this heading is for, if it is a heading.
    pub fn heading(&self) -> Option<usize> {
        match self.node_type {
            NodeType::Heading(group) => Some(group),
            _ => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn an_empty_directory_cannot_be_folded() {
        // Not a hypothetical: a filter can empty a directory that had files
        // in it, and a fold marker beside nothing reads as a broken row.
        let empty = Node::new("src", NodeType::Directory);
        assert!(!empty.is_foldable());

        let mut full = Node::new("src", NodeType::Directory);
        full.children.push(NodeId(1));
        assert!(full.is_foldable());
    }

    #[test]
    fn only_a_file_row_stands_for_an_entry() {
        assert_eq!(
            Node::new("a.rs", NodeType::File(EntryId { group: 0, file: 3 })).entry(),
            Some(EntryId { group: 0, file: 3 })
        );
        assert_eq!(Node::new("src", NodeType::Directory).entry(), None);
    }
}
