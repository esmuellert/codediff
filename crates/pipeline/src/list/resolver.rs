//! Deciding what to compare, and therefore which git command to run.
//!
//! The first stage, and the only place a revision the reader typed becomes an
//! id. `HEAD~3` is resolved **once**: a commit made while a review is open
//! must not leave half the files named against one `HEAD` and half against
//! another.
//!
//! Nothing here lists a file. What it produces is a plan — the arguments to
//! `git diff`, and what those arguments mean in the reviewer's terms.

use anyhow::{Context, Result};
use explorer::{ExplorerDiffRequest, ExplorerDiffType};
use file_types::{Rev, Revs};
use vcs::Git;

/// A repository, and what to ask it for.
pub struct Resolved {
    pub git: Git,
    pub plan: Plan,
}

/// Which git command answers this request.
///
/// Two shapes, because git has two: the working tree is described by a status,
/// and everything else by a diff. A status describes three things at once and
/// so yields two comparisons; a diff describes two things and yields one.
pub enum Plan {
    /// `git status`, which the caller reads as two comparisons.
    Worktree,
    /// `git diff <args>`, one comparison.
    Diff {
        /// What a heading will say.
        name: &'static str,
        /// What goes after `diff`.
        args: Vec<String>,
        /// What those arguments mean in the reviewer's terms.
        revs: Revs,
    },
}

/// Answers stage one: what am I being asked to compare.
pub fn resolve(request: &ExplorerDiffRequest) -> Result<Resolved> {
    let mut git = Git::open(&request.repo).context("opening a repository")?;
    let plan = plan(&mut git, &request.diff_type)?;
    Ok(Resolved { git, plan })
}

fn plan(git: &mut Git, diff_type: &ExplorerDiffType) -> Result<Plan> {
    let resolve_one = |git: &Git, name: &str| -> Result<Rev> {
        let oid = git
            .resolve(name)
            .with_context(|| format!("resolving {name}"))?;
        Ok(Rev::Commit(oid))
    };

    Ok(match diff_type {
        ExplorerDiffType::Worktree => Plan::Worktree,
        ExplorerDiffType::Against(rev) => {
            let before = resolve_one(git, rev)?;
            Plan::Diff {
                name: "Changes",
                args: vec![rev.clone()],
                revs: Revs::new(before, Rev::Worktree),
            }
        }
        ExplorerDiffType::Between(a, b) => {
            let (before, after) = (resolve_one(git, a)?, resolve_one(git, b)?);
            Plan::Diff {
                name: "Changes",
                args: vec![a.clone(), b.clone()],
                revs: Revs::new(before, after),
            }
        }
        ExplorerDiffType::MergeBase(base, target) => {
            // Where the two parted, which is what `a...b` means and the only
            // reason this is a variant rather than a spelling.
            let base = git
                .merge_base(base, target)
                .with_context(|| format!("finding where {base} and {target} parted"))?;
            let after = resolve_one(git, target)?;
            Plan::Diff {
                name: "Changes",
                args: vec![base.as_str().to_owned(), target.clone()],
                revs: Revs::new(Rev::Commit(base), after),
            }
        }
        ExplorerDiffType::Staged(rev) => {
            let before = resolve_one(git, rev)?;
            Plan::Diff {
                name: "Staged Changes",
                args: vec!["--cached".to_owned(), rev.clone()],
                revs: Revs::new(before, Rev::Index),
            }
        }
    })
}
