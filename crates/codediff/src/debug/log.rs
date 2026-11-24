//! Prints commit history as the repository layer sees it.

use anyhow::{Context, Result};
use vcs::{CommitOrder, Repository};

use crate::cli::LogOrder;
use crate::text::sanitize;

pub fn run(dir: &str, order: LogOrder, limit: usize) -> Result<()> {
    let commit_order = match order {
        LogOrder::Topo => CommitOrder::Topological,
        LogOrder::Date => CommitOrder::CommitDate,
        LogOrder::Author => CommitOrder::AuthorDate,
    };
    let repository = Repository::open(std::path::Path::new(dir))
        .with_context(|| format!("opening a repository at {dir}"))?;
    let commits = repository
        .list_commits(commit_order, limit)
        .with_context(|| format!("reading commit history at {dir}"))?;

    if commits.is_empty() {
        println!("no commits");
        return Ok(());
    }

    for commit in &commits {
        let marker = if commit.current { "*" } else { " " };
        let parents = if commit.parents.is_empty() {
            "-".to_owned()
        } else {
            commit
                .parents
                .iter()
                .map(|parent| parent.as_str())
                .collect::<Vec<_>>()
                .join(",")
        };
        let mut labels = Vec::new();
        for branch in &commit.branches {
            labels.push(format!("branch:{}", sanitize(branch)));
        }
        for remote in &commit.remotes {
            labels.push(format!("remote:{}", sanitize(remote)));
        }
        for tag in &commit.tags {
            labels.push(format!("tag:{}", sanitize(tag)));
        }
        let suffix = if labels.is_empty() {
            String::new()
        } else {
            format!(" [{}]", labels.join(", "))
        };
        println!(
            "{marker} {} ({parents}) {} {} {}{suffix}",
            sanitize(commit.hash.as_str()),
            sanitize(&commit.date),
            sanitize(&commit.author),
            sanitize(&commit.subject),
        );
    }
    Ok(())
}
