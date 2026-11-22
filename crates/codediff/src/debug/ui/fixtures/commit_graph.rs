//! Semantic construction of commit graph fixtures.

use file_types::Oid;
use vcs::Commit;

pub struct CommitBuilder {
    commit: Commit,
}

impl CommitBuilder {
    pub fn new(oid: &str, parents: &[&str], date: &str, author: &str, subject: &str) -> Self {
        Self {
            commit: Commit {
                oid: Oid::new(oid),
                parents: parents.iter().copied().map(Oid::new).collect(),
                current: false,
                branches: Vec::new(),
                remotes: Vec::new(),
                tags: Vec::new(),
                subject: subject.to_owned(),
                author: author.to_owned(),
                date: date.to_owned(),
            },
        }
    }

    pub fn current(mut self) -> Self {
        self.commit.current = true;
        self
    }

    pub fn branches(mut self, names: &[&str]) -> Self {
        self.commit.branches = names.iter().map(|s| (*s).to_owned()).collect();
        self
    }

    pub fn remotes(mut self, names: &[&str]) -> Self {
        self.commit.remotes = names.iter().map(|s| (*s).to_owned()).collect();
        self
    }

    pub fn tags(mut self, names: &[&str]) -> Self {
        self.commit.tags = names.iter().map(|s| (*s).to_owned()).collect();
        self
    }

    pub fn build(self) -> Commit {
        self.commit
    }
}
