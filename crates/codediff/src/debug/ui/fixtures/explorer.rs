//! Semantic construction of Explorer file lists.

use file_types::{ChangeType, File, Oid, Rev, Revs, Stats};

use super::{repo_path, worktree_revs};

#[derive(Default)]
pub struct ExplorerFixture {
    files: Vec<File>,
}

impl ExplorerFixture {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn changes(mut self, configure: impl FnOnce(&mut FileGroupBuilder<'_>)) -> Self {
        let mut group = FileGroupBuilder {
            files: &mut self.files,
            revs: worktree_revs(),
        };
        configure(&mut group);
        self
    }

    pub fn staged(mut self, configure: impl FnOnce(&mut FileGroupBuilder<'_>)) -> Self {
        let mut group = FileGroupBuilder {
            files: &mut self.files,
            revs: Revs::new(Rev::Commit(Oid::new("story-base")), Rev::Index),
        };
        configure(&mut group);
        self
    }

    pub fn build(self) -> Vec<File> {
        self.files
    }
}

pub struct FileGroupBuilder<'a> {
    files: &'a mut Vec<File>,
    revs: Revs,
}

impl FileGroupBuilder<'_> {
    pub fn modified(&mut self, path: &str, added: u32, removed: u32) -> &mut Self {
        self.push(
            File::unchanged_path(repo_path(path), self.revs.clone()),
            added,
            removed,
        )
    }

    pub fn added(&mut self, path: &str, added: u32) -> &mut Self {
        self.push(File::added(repo_path(path), self.revs.clone()), added, 0)
    }

    pub fn deleted(&mut self, path: &str, removed: u32) -> &mut Self {
        self.push(
            File::deleted(repo_path(path), self.revs.clone()),
            0,
            removed,
        )
    }

    pub fn untracked(&mut self, path: &str) -> &mut Self {
        self.files.push(
            File::added(repo_path(path), self.revs.clone()).set_change_type(ChangeType::Untracked),
        );
        self
    }

    pub fn renamed(
        &mut self,
        original: &str,
        modified: &str,
        added: u32,
        removed: u32,
    ) -> &mut Self {
        self.push(
            File::renamed(repo_path(original), repo_path(modified), self.revs.clone()),
            added,
            removed,
        )
    }

    pub fn conflicted(&mut self, path: &str) -> &mut Self {
        self.files.push(
            File::unchanged_path(repo_path(path), self.revs.clone())
                .set_change_type(ChangeType::Conflicted),
        );
        self
    }

    fn push(&mut self, file: File, added: u32, removed: u32) -> &mut Self {
        self.files.push(file.set_stats(Stats::new(added, removed)));
        self
    }
}
