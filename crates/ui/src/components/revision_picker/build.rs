//! Pure candidate and display-row construction for [`super::RevisionPicker`].

use file_types::Rev;

use crate::Commit;

/// One row in the scrollable revision list.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) enum DisplayRow {
    Worktree {
        candidate_index: usize,
    },
    Separator,
    Commit {
        commit_index: usize,
        candidate_index: usize,
    },
}

/// Selectable candidates and the display rows that frame them.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct RevisionList {
    pub candidates: Vec<Rev>,
    pub rows: Vec<DisplayRow>,
}

impl RevisionList {
    pub fn row_for_candidate(&self, candidate: usize) -> Option<usize> {
        self.rows.iter().position(|row| match row {
            DisplayRow::Worktree { candidate_index }
            | DisplayRow::Commit {
                candidate_index, ..
            } => *candidate_index == candidate,
            DisplayRow::Separator => false,
        })
    }

    pub fn candidate_at_row(&self, row_index: usize) -> Option<usize> {
        match self.rows.get(row_index)? {
            DisplayRow::Worktree { candidate_index }
            | DisplayRow::Commit {
                candidate_index, ..
            } => Some(*candidate_index),
            DisplayRow::Separator => None,
        }
    }
}

pub(super) fn build_list(
    commits: &[Commit],
    worktree_summary: Option<&str>,
    query: &str,
) -> RevisionList {
    let needle = query.trim().to_lowercase();
    let filtering = !needle.is_empty();

    let mut candidates = Vec::new();
    let mut rows = Vec::new();

    if let Some(summary) = worktree_summary
        && (!filtering || worktree_matches(summary, &needle))
    {
        let candidate_index = candidates.len();
        candidates.push(Rev::Worktree);
        rows.push(DisplayRow::Worktree { candidate_index });
    }

    let matching_commits: Vec<usize> = commits
        .iter()
        .enumerate()
        .filter(|(_, commit)| !filtering || commit_matches(commit, &needle))
        .map(|(index, _)| index)
        .collect();

    if !matching_commits.is_empty() {
        if !rows.is_empty() {
            rows.push(DisplayRow::Separator);
        }
        for commit_index in matching_commits {
            let candidate_index = candidates.len();
            candidates.push(Rev::Commit(commits[commit_index].oid.clone()));
            rows.push(DisplayRow::Commit {
                commit_index,
                candidate_index,
            });
        }
    }

    RevisionList { candidates, rows }
}

fn worktree_matches(summary: &str, needle: &str) -> bool {
    contains_lowercase("working tree worktree", needle) || contains_lowercase(summary, needle)
}

fn commit_matches(commit: &Commit, needle: &str) -> bool {
    if commit.current && "head".contains(needle) {
        return true;
    }
    contains_lowercase(commit.oid.as_str(), needle)
        || commit
            .branches
            .iter()
            .any(|branch| contains_lowercase(branch, needle))
        || commit
            .remotes
            .iter()
            .any(|remote| contains_lowercase(remote, needle))
        || commit.tags.iter().any(|tag| {
            contains_lowercase(tag, needle) || contains_lowercase(&format!("tag: {tag}"), needle)
        })
        || contains_lowercase(&commit.subject, needle)
        || contains_lowercase(&commit.author, needle)
        || contains_lowercase(&commit.date, needle)
}

fn contains_lowercase(haystack: &str, needle_lower: &str) -> bool {
    haystack.to_lowercase().contains(needle_lower)
}
