//! Which version of a file's content.
//!
//! ---
//!
//! Git can name a file's content five ways, and a reviewer needs a sixth that
//! git has no name for:
//!
//! ```text
//! <rev>:<path>    a blob in a commit
//! :0:<path>       the index — what `git add` put there
//! :1:<path>       merge stage 1, the common ancestor
//! :2:<path>       merge stage 2, ours
//! :3:<path>       merge stage 3, theirs
//! (the file)      on disk, which git has never hashed
//! ```
//!
//! **This is a name, not a hash.** [`Rev::Index`] means "whatever `git add`
//! last put there", which is different bytes after every `git add`. Only
//! [`Rev::Commit`] is stable, because a commit cannot change. So none of the
//! four carries a stamp: giving one to the working tree and not to the index
//! would be arbitrary, and giving one to all of them would make this a hash
//! rather than a name — and a name is what a status line prints and an
//! explorer groups by.
//!
//! Whether those bytes have changed *since they were read* is a different
//! question with a different answer, and [`can_change`](Rev::can_change) is
//! how the two meet.

use crate::Oid;

/// Which version of a file's content.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum Rev {
    /// The file on disk — what an editor would open.
    ///
    /// The default after side, and the only version git does not store.
    Worktree,
    /// The index: what `git add` put there. Git's stage 0.
    ///
    /// Cannot coexist with [`Conflict`](Rev::Conflict): a path has stage 0, or
    /// stages 1 to 3, never both.
    Index,
    /// One side of an unresolved merge. Git's stages 1 to 3.
    Conflict(Stage),
    /// A commit, by its id.
    ///
    /// An id rather than `HEAD`, because `HEAD` moves — a name that moves
    /// cannot say which bytes were read.
    Commit(Oid),
}

/// Which side of an unresolved merge, by git's stage numbers.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Stage {
    /// Stage 1 — the common ancestor, before either side touched it.
    Base,
    /// Stage 2 — the branch being merged into. Usually yours.
    Ours,
    /// Stage 3 — the branch being merged in.
    Theirs,
}

impl Rev {
    /// How git names this revision, before the `:path`.
    ///
    /// `None` for the working tree, which is not in the object store at all
    /// and is read from disk instead — so one call answers both "where does
    /// this come from" and "what do I pass to git".
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

    /// Whether these bytes can change while a reader is looking at them.
    ///
    /// False only for a commit. What a file watcher acts on, and what decides
    /// whether anything read from here may be kept for good.
    pub fn can_change(&self) -> bool {
        !matches!(self, Rev::Commit(_))
    }
}

impl std::fmt::Display for Rev {
    /// For a reader, not for git. See [`stored`](Rev::stored) for that.
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
        // These strings go straight to `git cat-file`, which reads
        // `<rev>:<path>`. A wrong one is a file that silently reads as
        // another version of itself.
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
        // The index changes on `git add`, a conflict stage when the merge is
        // resolved, and the working tree when a file is saved.
        assert!(Rev::Worktree.can_change());
        assert!(Rev::Index.can_change());
        assert!(Rev::Conflict(Stage::Ours).can_change());
        assert!(!Rev::Commit(Oid::new("abc123")).can_change());
    }

    #[test]
    fn revisions_of_one_path_are_told_apart() {
        // The whole reason this exists: the staged and the on-disk version of
        // one file are different bytes, and a cache that mixed them would
        // colour one with the other's answer.
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
