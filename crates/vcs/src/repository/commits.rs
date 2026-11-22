//! Lists commit history as VCS-agnostic [`Commit`] records.

use file_types::Oid;

use crate::git::log::{self, Entry, Order};

use super::Repository;

/// Ordering for [`Repository::list_commits`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum CommitOrder {
    /// Keeps branches grouped together (`--topo-order`).
    #[default]
    Topological,
    /// Orders by committer timestamp while keeping parents below children (`--date-order`).
    CommitDate,
    /// Orders by author timestamp while keeping parents below children (`--author-date-order`).
    AuthorDate,
}

impl CommitOrder {
    fn git_order(self) -> Order {
        match self {
            CommitOrder::Topological => Order::Topological,
            CommitOrder::CommitDate => Order::CommitDate,
            CommitOrder::AuthorDate => Order::AuthorDate,
        }
    }
}

/// One commit in repository history.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Commit {
    pub oid: Oid,
    pub parents: Vec<Oid>,
    /// Whether this commit is currently checked out.
    pub current: bool,
    pub branches: Vec<String>,
    pub remotes: Vec<String>,
    pub tags: Vec<String>,
    pub subject: String,
    pub author: String,
    pub date: String,
}

impl Repository {
    /// Lists up to `limit` commits in `order`.
    pub fn list_commits(&self, order: CommitOrder, limit: usize) -> crate::Result<Vec<Commit>> {
        Ok(log::list(&self.repo, order.git_order(), limit)?
            .into_iter()
            .map(entry_to_commit)
            .collect())
    }
}

fn entry_to_commit(entry: Entry) -> Commit {
    let mut current = false;
    let mut branches = Vec::new();
    let mut remotes = Vec::new();
    let mut tags = Vec::new();

    for decoration in entry.decorations {
        if decoration == "HEAD" {
            current = true;
        } else if let Some(target) = decoration.strip_prefix("HEAD -> ") {
            current = true;
            if let Some(branch) = target.strip_prefix("refs/heads/") {
                branches.push(branch.to_owned());
            } else {
                branches.push(target.to_owned());
            }
        } else if let Some(tag) = decoration.strip_prefix("tag: ") {
            let name = tag.strip_prefix("refs/tags/").unwrap_or(tag);
            tags.push(name.to_owned());
        } else if let Some(branch) = decoration.strip_prefix("refs/heads/") {
            branches.push(branch.to_owned());
        } else if let Some(remote) = decoration.strip_prefix("refs/remotes/")
            && !remote.ends_with("/HEAD")
        {
            remotes.push(remote.to_owned());
        }
    }

    Commit {
        oid: Oid::new(entry.oid),
        parents: entry.parents.into_iter().map(Oid::new).collect(),
        current,
        branches,
        remotes,
        tags,
        subject: entry.subject,
        author: entry.author,
        date: entry.date,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn entry(decorations: &[&str]) -> Entry {
        Entry {
            oid: "8339837".to_owned(),
            parents: vec!["09d43b9".to_owned()],
            decorations: decorations.iter().map(|s| (*s).to_owned()).collect(),
            subject: "subject".to_owned(),
            author: "Author".to_owned(),
            date: "2026-09-09".to_owned(),
        }
    }

    #[test]
    fn strips_git_decoration_prefixes_and_marks_checked_out_branch() {
        let commit = entry_to_commit(entry(&[
            "HEAD -> refs/heads/feat/revs",
            "tag: refs/tags/v1.0.0",
            "refs/remotes/origin/main",
            "refs/remotes/origin/HEAD",
            "refs/heads/main",
        ]));
        assert!(commit.current);
        assert_eq!(commit.branches, vec!["feat/revs", "main"]);
        assert_eq!(commit.remotes, vec!["origin/main"]);
        assert_eq!(commit.tags, vec!["v1.0.0"]);
    }

    #[test]
    fn detached_checkout_marks_current_without_inventing_a_branch() {
        let commit = entry_to_commit(entry(&[
            "HEAD",
            "refs/remotes/upstream/feat/topic",
            "refs/heads/origin-local",
        ]));
        assert!(commit.current);
        assert_eq!(commit.branches, vec!["origin-local"]);
        assert_eq!(commit.remotes, vec!["upstream/feat/topic"]);
        assert!(commit.tags.is_empty());
    }
}
