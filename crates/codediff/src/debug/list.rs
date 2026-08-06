//! `codediff debug list` — the groups a request produces.
//!
//! The list pipeline, printed. `debug status` shows what git said; this shows
//! what the explorer will be handed, which is the other side of the
//! translation and the only place the two can be compared.
//!
//! Machine-readable on purpose: one line per group and one per file, so a test
//! can assert on it without a terminal.

use anyhow::Result;
use explorer::{ExplorerDiffRequest, ExplorerDiffType};

/// The words `debug list` takes, as a diff type.
///
/// It lives here because **only this subcommand reaches it**. The command line
/// has no way to say any of it, and is not going to grow one: a reviewer
/// should not have to know git's revision syntax to open a review. `a...b` is
/// exact and almost nobody knows it. What compares against what is a decision
/// made *inside* the review, where it can be shown and changed, the way
/// lazygit does it. See D62.
///
/// The types themselves are real and reachable — the list pipeline resolves
/// all five — so the interface has something to switch between when the keys
/// for it arrive. This is how a test reaches them without a terminal.
pub fn diff_type(rev: &[String], staged: bool) -> ExplorerDiffType {
    use ExplorerDiffType as Type;
    match (staged, rev.first().cloned(), rev.get(1).cloned()) {
        // `--staged` with no revision means against the last commit, which is
        // what `git diff --cached` means with no revision.
        (true, rev, _) => Type::Staged(rev.unwrap_or_else(|| "HEAD".to_owned())),
        (false, None, _) => Type::Worktree,
        (false, Some(a), Some(b)) => Type::Between(a, b),
        (false, Some(a), None) => match a.split_once("...") {
            Some((base, target)) => Type::MergeBase(base.to_owned(), target.to_owned()),
            None => Type::Against(a),
        },
    }
}

/// Prints every group, and every file in it.
pub fn run(diff_type: ExplorerDiffType, pathspec: Vec<String>) -> Result<()> {
    let cwd = std::env::current_dir()?;
    let root = vcs::Git::open(&cwd)?.repo().root.clone();
    let request = ExplorerDiffRequest::new(root, diff_type).with_pathspec(pathspec);

    for group in pipeline::list::run(&request)? {
        // The revisions, not only the name: a name is a label a human reads,
        // and what the group *is* is the pair.
        println!(
            "group {:?} {} -> {}",
            group.name, group.revs.before, group.revs.after
        );
        for entry in &group.files {
            let stats = match entry.stats {
                Some(stats) => format!(" +{} -{}", stats.added, stats.removed),
                None => String::new(),
            };
            println!("  {} {}{stats}", entry.status(), entry.path());
        }
    }
    Ok(())
}
