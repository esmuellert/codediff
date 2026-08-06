//! Files that share a comparison.
//!
//! **A group is a revision pair.** "Staged Changes" is not a category a file
//! belongs to — it is the name for comparing the index against a commit, and
//! every file in it already carries that pair. Keeping the category as well
//! would be keeping the same fact twice, which is how two copies come to
//! disagree.
//!
//! The Neovim plugin kept it twice, as a struct of `unstaged` and `staged`
//! lists, and its own source records what that cost: comparing two revisions
//! produced files that were neither, and it wrote
//!
//! ```text
//! -- For revision comparison, we treat everything as "unstaged" for explorer
//! -- compatibility
//! ```
//!
//! See D57.

use file_types::Revs;

use crate::Entry;

/// Files that are all the same comparison.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Group {
    /// What the heading says.
    ///
    /// A name rather than a variant, because a comparison of two revisions has
    /// no fixed name — it is whatever describes those revisions. Chosen by
    /// whatever ran git, which is the only layer that knows both the reader's
    /// words and git's.
    pub name: String,
    /// The comparison this group *is*.
    pub revs: Revs,
    pub files: Vec<Entry>,
}

impl Group {
    pub fn new(name: impl Into<String>, revs: Revs, files: Vec<Entry>) -> Self {
        Self {
            name: name.into(),
            revs,
            files,
        }
    }

    pub fn is_empty(&self) -> bool {
        self.files.is_empty()
    }
}

/// Every group a request produced, in the order they are shown.
pub type Groups = Vec<Group>;

#[cfg(test)]
mod tests {
    use super::*;
    use file_types::{ChangedFile, File, Oid, RepoPath, Rev};
    use std::path::Path;

    #[test]
    fn a_group_carries_the_comparison_its_files_carry() {
        // The property the deleted `Section` could not have: there is one
        // answer to "what is this compared against", so nothing can disagree.
        let revs = Revs::new(Rev::Commit(Oid::new("b87b24c")), Rev::Index);
        let path = RepoPath::new("a.rs", Path::new("/repo"));
        let file = ChangedFile::new(File::unchanged_path(path, revs.clone()), None);
        let group = Group::new("Staged Changes", revs.clone(), vec![Entry::new(file)]);

        let shown = &group.files[0].file.file;
        assert_eq!(
            shown.rev(file_types::DiffVersion::Original),
            &group.revs.before
        );
        assert_eq!(
            shown.rev(file_types::DiffVersion::Modified),
            &group.revs.after
        );
    }
}
