use anyhow::Result;
use loom::crokey::{KeyCombination, key};
use vcs::Commit;

use super::super::definition::{StoryComponent, StoryDefinition, StoryFixture};
use super::super::fixtures::commit_graph::CommitBuilder;

const SEARCH_INITIAL_KEYS: &[KeyCombination] = &[
    key!('/'),
    key!(v),
    key!('0'),
    key!('.'),
    key!('2'),
    key!('8'),
];

pub const STORIES: &[StoryDefinition] = &[
    story(
        "revision-picker/default",
        "Pinned Working tree above a divider and seamless U+F5D0..U+F60D commit graph",
        &[],
        branch_drawing_picker,
    ),
    story(
        "revision-picker/box-fallback",
        "Box-drawing fallback (○/◎ + ╭ ╮ ╰ ╯ │ ─) for terminals without U+F5D0..U+F60D",
        &[],
        box_fallback_picker,
    ),
    story(
        "revision-picker/search",
        "Open / search bar packing distant matching tags into a flat list",
        SEARCH_INITIAL_KEYS,
        branch_drawing_picker,
    ),
];

const fn story(
    id: &'static str,
    description: &'static str,
    initial_keys: &'static [KeyCombination],
    make_fixture: fn() -> Result<StoryFixture>,
) -> StoryDefinition {
    StoryDefinition {
        id,
        description,
        component: StoryComponent::RevisionPicker,
        compact: false,
        snapshot_size: (110, 24),
        initial_keys,
        make_fixture,
    }
}

fn branch_drawing_picker() -> Result<StoryFixture> {
    Ok(StoryFixture::RevisionPicker {
        commits: branching_history(),
        worktree_files: 6,
        branch_drawing: true,
    })
}

fn box_fallback_picker() -> Result<StoryFixture> {
    Ok(StoryFixture::RevisionPicker {
        commits: branching_history(),
        worktree_files: 6,
        branch_drawing: false,
    })
}

/// Authentic `git log --topo-order` commit DAG captured from a real Git repository.
fn branching_history() -> Vec<Commit> {
    vec![
        CommitBuilder::new(
            "fb0d76c",
            &["91dba92"],
            "2026-09-10",
            "github-actions[bot]",
            "release: v0.29.0 [skip ci]",
        )
        .current()
        .branches(&["main"])
        .remotes(&["origin/main"])
        .tags(&["v0.29.0"])
        .build(),
        CommitBuilder::new(
            "91dba92",
            &["3493f95", "f8b977f"],
            "2026-09-09",
            "Yanuo Ma",
            "refactor(ui): restore native scrollbind (#557)",
        )
        .build(),
        CommitBuilder::new(
            "f8b977f",
            &["3493f95"],
            "2026-09-09",
            "Yanuo Ma",
            "refactor(ui): restore native scrollbind",
        )
        .branches(&["feat/scrollbind"])
        .remotes(&["origin/feat/scrollbind"])
        .build(),
        CommitBuilder::new(
            "3493f95",
            &["e878350"],
            "2026-09-09",
            "github-actions[bot]",
            "release: v0.28.4 [skip ci]",
        )
        .tags(&["v0.28.4"])
        .build(),
        CommitBuilder::new(
            "e878350",
            &["d9b9808", "ff41e8e"],
            "2026-09-09",
            "Yanuo Ma",
            "fix(conflict): draw complete block boundaries (#556)",
        )
        .build(),
        CommitBuilder::new(
            "ff41e8e",
            &["d9b9808"],
            "2026-09-08",
            "Yanuo Ma",
            "fix(conflict): draw complete block boundaries",
        )
        .remotes(&["origin/fix/gutter-boundaries"])
        .build(),
        CommitBuilder::new(
            "d9b9808",
            &["89480e6", "c06a123"],
            "2026-09-08",
            "Yanuo Ma",
            "fix(conflict): map u to undo result edits (#555)",
        )
        .build(),
        CommitBuilder::new(
            "c06a123",
            &["16dfa60"],
            "2026-09-07",
            "copilot-swe-agent[bot]",
            "fix(conflict): map u to undo result edits",
        )
        .remotes(&["origin/copilot/undo-edits"])
        .build(),
        CommitBuilder::new(
            "16dfa60",
            &["e252a06"],
            "2026-09-07",
            "copilot-swe-agent[bot]",
            "chore: initial plan for conflict undo",
        )
        .build(),
        CommitBuilder::new(
            "89480e6",
            &["d7b2373"],
            "2026-09-07",
            "github-actions[bot]",
            "release: v0.28.3 [skip ci]",
        )
        .tags(&["v0.28.3"])
        .build(),
        CommitBuilder::new(
            "d7b2373",
            &["e252a06", "fa1ba01"],
            "2026-09-07",
            "Yanuo Ma",
            "fix(watcher): coalesce rapid filesystem events (#554)",
        )
        .build(),
        CommitBuilder::new(
            "fa1ba01",
            &["e252a06"],
            "2026-09-07",
            "Yanuo Ma",
            "fix(watcher): coalesce rapid filesystem events",
        )
        .branches(&["fix/watcher-coalesce"])
        .build(),
        CommitBuilder::new(
            "e252a06",
            &["56d8b3d", "fc1b1fc"],
            "2026-09-06",
            "Yanuo Ma",
            "feat(explorer): stage directory fold transitions (#553)",
        )
        .build(),
        CommitBuilder::new(
            "fc1b1fc",
            &["0984d86"],
            "2026-09-06",
            "Yanuo Ma",
            "test(explorer): cover directory fold transitions",
        )
        .remotes(&["origin/feat/directory-folds"])
        .build(),
        CommitBuilder::new(
            "0984d86",
            &["eb877e5"],
            "2026-09-06",
            "Yanuo Ma",
            "refactor(explorer): extract fold key lookup",
        )
        .build(),
        CommitBuilder::new(
            "56d8b3d",
            &["52d47d5"],
            "2026-09-06",
            "Yanuo Ma",
            "docs: consolidate diff feature overview",
        )
        .build(),
        CommitBuilder::new(
            "52d47d5",
            &["ff37098"],
            "2026-09-06",
            "github-actions[bot]",
            "release: v0.28.2 [skip ci]",
        )
        .tags(&["v0.28.2"])
        .build(),
        CommitBuilder::new(
            "ff37098",
            &["eb877e5", "d9256cb"],
            "2026-09-05",
            "Yanuo Ma",
            "feat(syntax): highlight markdown fenced blocks (#552)",
        )
        .build(),
        CommitBuilder::new(
            "d9256cb",
            &["eb877e5"],
            "2026-09-05",
            "Yanuo Ma",
            "feat(syntax): highlight markdown fenced blocks",
        )
        .remotes(&["origin/feat/markdown-fences"])
        .build(),
        CommitBuilder::new(
            "eb877e5",
            &["af1c317"],
            "2026-09-05",
            "github-actions[bot]",
            "release: v0.28.1 [skip ci]",
        )
        .tags(&["v0.28.1"])
        .build(),
        CommitBuilder::new(
            "af1c317",
            &["d9d0c13", "01b1b6a"],
            "2026-09-05",
            "Yanuo Ma",
            "refactor(loom): simplify scroll viewport clamping (#551)",
        )
        .build(),
        CommitBuilder::new(
            "01b1b6a",
            &["d9d0c13"],
            "2026-09-05",
            "Yanuo Ma",
            "refactor(loom): simplify scroll viewport clamping",
        )
        .build(),
        CommitBuilder::new(
            "d9d0c13",
            &["aab6b86"],
            "2026-09-04",
            "github-actions[bot]",
            "release: v0.28.0 [skip ci]",
        )
        .tags(&["v0.28.0"])
        .build(),
        CommitBuilder::new(
            "aab6b86",
            &[],
            "2026-09-03",
            "Yanuo Ma",
            "chore: bootstrap workspace crates",
        )
        .build(),
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_and_box_fallback_share_commits_with_distinct_symbol_modes() {
        let StoryFixture::RevisionPicker {
            commits: branch_commits,
            branch_drawing: true,
            ..
        } = branch_drawing_picker().unwrap()
        else {
            unreachable!()
        };
        let StoryFixture::RevisionPicker {
            commits: box_commits,
            branch_drawing: false,
            ..
        } = box_fallback_picker().unwrap()
        else {
            unreachable!()
        };
        assert_eq!(branch_commits.len(), 24);
        assert_eq!(branch_commits, box_commits);
        assert!(branch_commits[0].current);
        assert_eq!(branch_commits[0].branches, vec!["main"]);
        assert_eq!(branch_commits[0].remotes, vec!["origin/main"]);
        assert_eq!(branch_commits[0].tags, vec!["v0.29.0"]);
        assert!(
            branch_commits
                .iter()
                .any(|commit| commit.parents.len() >= 2)
        );
        assert!(branch_commits.last().unwrap().parents.is_empty());
    }
}
