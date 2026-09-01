//! Versions from which file content can be read.

use crate::Oid;

/// Which version of a file's content.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum Rev {
    /// The file on disk.
    Worktree,
    /// Git's stage 0 index.
    Index,
    /// One side of an unresolved merge. Git's stages 1 to 3.
    Conflict(Stage),
    /// A commit identified by its immutable id.
    Commit(Oid),
}

/// Which side of an unresolved merge, by git's stage numbers.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Stage {
    /// Stage 1, the common ancestor.
    Base,
    /// Stage 2, the branch being merged into.
    Ours,
    /// Stage 3, the branch being merged.
    Theirs,
}

impl Rev {
    /// Git's revision prefix, or `None` for the working tree.
    pub fn stored(&self) -> Option<&str> {
        Some(match self {
            Rev::Worktree => return None,
            Rev::Index => ":0",
            Rev::Conflict(Stage::Base) => ":1",
            Rev::Conflict(Stage::Ours) => ":2",
            Rev::Conflict(Stage::Theirs) => ":3",
            Rev::Commit(oid) => oid.as_str(),
        })
    }

    /// Whether this revision can change during a review.
    pub fn can_change(&self) -> bool {
        !matches!(self, Rev::Commit(_))
    }
}

impl std::fmt::Display for Rev {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Rev::Worktree => f.write_str("working tree"),
            Rev::Index => f.write_str("staged"),
            Rev::Conflict(Stage::Base) => f.write_str("common ancestor"),
            Rev::Conflict(Stage::Ours) => f.write_str("ours"),
            Rev::Conflict(Stage::Theirs) => f.write_str("theirs"),
            Rev::Commit(oid) => write!(f, "{oid}"),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn each_stored_revision_spells_itself_the_way_git_does() {
        assert_eq!(Rev::Index.stored(), Some(":0"));
        assert_eq!(Rev::Conflict(Stage::Base).stored(), Some(":1"));
        assert_eq!(Rev::Conflict(Stage::Ours).stored(), Some(":2"));
        assert_eq!(Rev::Conflict(Stage::Theirs).stored(), Some(":3"));
        assert_eq!(Rev::Commit(Oid::new("abc123")).stored(), Some("abc123"));
    }

    #[test]
    fn the_working_tree_is_the_one_git_cannot_name() {
        assert_eq!(Rev::Worktree.stored(), None);
    }

    #[test]
    fn only_a_commit_cannot_change() {
        assert!(Rev::Worktree.can_change());
        assert!(Rev::Index.can_change());
        assert!(Rev::Conflict(Stage::Ours).can_change());
        assert!(!Rev::Commit(Oid::new("abc123")).can_change());
    }

    #[test]
    fn revisions_of_one_path_are_told_apart() {
        assert_ne!(Rev::Worktree, Rev::Index);
        assert_ne!(Rev::Conflict(Stage::Ours), Rev::Conflict(Stage::Theirs));
        assert_ne!(
            Rev::Commit(Oid::new("aaa")),
            Rev::Commit(Oid::new("bbb")),
            "two commits are two versions"
        );
    }

    #[test]
    fn each_revision_says_what_it_is_in_words() {
        assert_eq!(Rev::Worktree.to_string(), "working tree");
        assert_eq!(Rev::Index.to_string(), "staged");
        assert_eq!(Rev::Conflict(Stage::Theirs).to_string(), "theirs");
        assert_eq!(Rev::Commit(Oid::new("abc123")).to_string(), "abc123");
    }
}
