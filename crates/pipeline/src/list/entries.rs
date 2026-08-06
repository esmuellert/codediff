//! Running the command, and saying the answer in the explorer's words.
//!
//! The second stage, and the only place git's vocabulary and the explorer's
//! are both spoken. `explorer` may not name `vcs` and `vcs` may not name
//! `explorer` — `cargo xtask lint-arch` forbids both — so the translation
//! happens here, in the binary, exactly as `to_file_diff` does for one file.

use anyhow::{Context, Result};
use explorer::{Entry, ExplorerDiffRequest, Group, Groups};
use vcs::Git;
use vcs::git::{Changes, Counts};

use crate::list::resolver::{Plan, Resolved};

/// Answers stage two: go and get the files.
pub fn read(resolved: Resolved, request: &ExplorerDiffRequest) -> Result<Groups> {
    let Resolved { mut git, plan } = resolved;
    let changes = match &plan {
        Plan::Worktree => git
            .worktree_changes(&request.pathspec)
            .context("listing changed files")?,
        Plan::Diff { name, args, revs } => {
            let args: Vec<&str> = args.iter().map(String::as_str).collect();
            git.diff_changes(name, &args, revs.clone(), &request.pathspec)
                .context("listing changed files")?
        }
    };

    let counts = counts(&git, &plan, request);
    Ok(changes
        .into_iter()
        .filter(|group| !group.is_empty())
        .map(|group| translate(group, &counts))
        .collect())
}

/// Git's group, in the explorer's words.
///
/// The whole translation: a name, a revision pair, and files carrying their
/// line counts. Nothing is decided here — which groups exist was decided by
/// the plan, and what is in them by git.
fn translate(changes: Changes, counts: &Counts) -> Group {
    let files = changes
        .files
        .into_iter()
        .map(|file| {
            let stats = counts.get(file.path().as_str()).copied();
            let entry = Entry::new(file);
            match stats {
                Some(stats) => entry.with_stats(stats),
                None => entry,
            }
        })
        .collect();
    Group::new(changes.name, changes.revs, files)
}

/// How many lines each file gained and lost.
///
/// A failure to count is not a failure to review: the list is still correct
/// without the numbers, so a repository that will not answer loses the counts
/// rather than the whole screen.
///
/// One map for every group, keyed by path. Two groups can hold the same path —
/// a file staged and then edited again — and then one of them shows the
/// other's numbers. That is wrong, and it is the smaller wrong: keeping them
/// apart means a map per group, which is a third stage's worth of plumbing for
/// a number beside a name. Recorded rather than hidden.
fn counts(git: &Git, plan: &Plan, request: &ExplorerDiffRequest) -> Counts {
    match plan {
        Plan::Worktree => {
            let mut counts = git.unstaged_counts().unwrap_or_default();
            counts.extend(git.staged_counts().unwrap_or_default());
            counts
        }
        Plan::Diff { args, .. } => {
            let args: Vec<&str> = args.iter().map(String::as_str).collect();
            git.diff_counts(&args, &request.pathspec)
                .unwrap_or_default()
        }
    }
}
