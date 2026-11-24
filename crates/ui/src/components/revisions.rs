//! Pure revision-endpoint domain logic shared by `App`, `RevisionHeader`, and `RevisionPicker`.

use diff_service::files::RevisionRange;
use file_types::{Oid, Rev};

use crate::Commit;

/// Which endpoint pill is currently active.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RevisionSide {
    Left,
    Right,
}

/// Returns the currently checked-out commit, if present in `commits`.
pub fn head_commit(commits: &[Commit]) -> Option<&Commit> {
    commits.iter().find(|commit| commit.current)
}

/// Finds a commit matching `rev` by OID, OID prefix, branch, remote, or tag.
pub fn find_commit<'a>(rev: &str, commits: &'a [Commit]) -> Option<&'a Commit> {
    if rev == "HEAD" {
        return head_commit(commits);
    }
    commits.iter().find(|commit| {
        let oid = commit.oid.as_str();
        oid == rev
            || oid.starts_with(rev)
            || rev.starts_with(oid)
            || commit.branches.iter().any(|b| b == rev)
            || commit.remotes.iter().any(|r| r == rev)
            || commit.tags.iter().any(|t| t == rev)
    })
}

/// Swaps the two endpoints when both sides are commits.
pub fn swap_revision_range(range: &RevisionRange) -> Option<RevisionRange> {
    match range {
        RevisionRange::Between(from, to) => Some(RevisionRange::Between(to.clone(), from.clone())),
        RevisionRange::MergeBase(base, head) => {
            Some(RevisionRange::MergeBase(head.clone(), base.clone()))
        }
        RevisionRange::Worktree | RevisionRange::Against(_) | RevisionRange::Staged(_) => None,
    }
}

fn left_rev_string(range: &RevisionRange, commits: &[Commit]) -> String {
    match range {
        RevisionRange::Worktree => head_commit(commits)
            .map(|commit| commit.oid.as_str().to_owned())
            .unwrap_or_else(|| "HEAD".to_owned()),
        RevisionRange::Against(rev)
        | RevisionRange::Staged(rev)
        | RevisionRange::Between(rev, _)
        | RevisionRange::MergeBase(rev, _) => rev.clone(),
    }
}

/// Initial revision to highlight when opening the picker for `side`.
pub fn initial_for_side(
    side: RevisionSide,
    range: &RevisionRange,
    commits: &[Commit],
) -> Option<Rev> {
    match side {
        RevisionSide::Left => {
            let left = left_rev_string(range, commits);
            let oid = find_commit(&left, commits)
                .map(|commit| commit.oid.clone())
                .unwrap_or_else(|| Oid::new(left));
            Some(Rev::Commit(oid))
        }
        RevisionSide::Right => match range {
            RevisionRange::Worktree | RevisionRange::Against(_) => Some(Rev::Worktree),
            RevisionRange::Staged(_) => None,
            RevisionRange::Between(_, to) | RevisionRange::MergeBase(_, to) => {
                let oid = find_commit(to, commits)
                    .map(|commit| commit.oid.clone())
                    .unwrap_or_else(|| Oid::new(to));
                Some(Rev::Commit(oid))
            }
        },
    }
}

/// Applies a picked revision to `side` of `range`.
pub fn apply_picked_revision(
    side: RevisionSide,
    picked: Rev,
    range: &RevisionRange,
    commits: &[Commit],
) -> Option<RevisionRange> {
    let head_oid = head_commit(commits).map(|commit| commit.oid.as_str());
    match side {
        RevisionSide::Left => {
            let Rev::Commit(oid) = picked else {
                return None;
            };
            let picked_str = oid.as_str().to_owned();
            let is_head = head_oid == Some(picked_str.as_str());
            match range {
                RevisionRange::Worktree | RevisionRange::Against(_) => {
                    if is_head {
                        Some(RevisionRange::Worktree)
                    } else {
                        Some(RevisionRange::Against(picked_str))
                    }
                }
                RevisionRange::Staged(_) => Some(RevisionRange::Staged(picked_str)),
                RevisionRange::Between(_, to) | RevisionRange::MergeBase(_, to) => {
                    Some(RevisionRange::Between(picked_str, to.clone()))
                }
            }
        }
        RevisionSide::Right => {
            let left = left_rev_string(range, commits);
            let left_is_head = matches!(range, RevisionRange::Worktree)
                || left == "HEAD"
                || find_commit(&left, commits).is_some_and(|commit| commit.current);
            match picked {
                Rev::Worktree => {
                    if left_is_head {
                        Some(RevisionRange::Worktree)
                    } else {
                        Some(RevisionRange::Against(left))
                    }
                }
                Rev::Commit(oid) => Some(RevisionRange::Between(left, oid.as_str().to_owned())),
                Rev::Index | Rev::Conflict(_) => None,
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn picking_worktree_on_right_when_left_is_named_head_branch_returns_worktree() {
        let commits = vec![
            Commit {
                oid: Oid::new("6152138abcdef"),
                parents: vec![Oid::new("fb0d76c111111")],
                current: true,
                branches: vec!["main".to_owned()],
                remotes: Vec::new(),
                tags: Vec::new(),
                subject: "head".to_owned(),
                author: "Alex".to_owned(),
                date: "now".to_owned(),
            },
            Commit {
                oid: Oid::new("fb0d76c111111"),
                parents: Vec::new(),
                current: false,
                branches: Vec::new(),
                remotes: Vec::new(),
                tags: vec!["v0.1.0".to_owned()],
                subject: "old".to_owned(),
                author: "Alex".to_owned(),
                date: "yesterday".to_owned(),
            },
        ];
        let range = RevisionRange::Between("main".to_owned(), "v0.1.0".to_owned());
        assert_eq!(
            apply_picked_revision(RevisionSide::Right, Rev::Worktree, &range, &commits),
            Some(RevisionRange::Worktree)
        );
    }
}
