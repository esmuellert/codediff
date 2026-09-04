use anyhow::Result;
use loom::crokey::{KeyCombination, key};

use super::super::definition::{StoryDefinition, StoryFixture, StoryType};
use super::super::fixtures::explorer::ExplorerFixture;

const LIST_SETUP: &[KeyCombination] = &[key!(i)];
const FOLDED_SETUP: &[KeyCombination] = &[key!(j), key!(enter)];
const SELECTED_SETUP: &[KeyCombination] = &[key!(j), key!(j), key!(j)];

pub const STORIES: &[StoryDefinition] = &[
    story("explorer/empty", "No changed files", &[], empty),
    story(
        "explorer/tree",
        "Nested changed files in tree mode",
        &[],
        canonical,
    ),
    story(
        "explorer/list",
        "The same files in flat-list mode",
        LIST_SETUP,
        canonical,
    ),
    story(
        "explorer/folded",
        "A directory collapsed through Enter",
        FOLDED_SETUP,
        canonical,
    ),
    story(
        "explorer/selected",
        "A file selected through keyboard navigation",
        SELECTED_SETUP,
        canonical,
    ),
    story(
        "explorer/long-list",
        "Enough files to exercise viewport scrolling",
        &[],
        long_list,
    ),
    story(
        "explorer/mixed-status",
        "Every change type across working-tree and staged groups",
        &[],
        mixed_status,
    ),
    story(
        "explorer/awkward-paths",
        "Deep, repeated, spaced, long, and Unicode paths",
        &[],
        awkward_paths,
    ),
];

const fn story(
    id: &'static str,
    summary: &'static str,
    setup: &'static [KeyCombination],
    build: fn() -> Result<StoryFixture>,
) -> StoryDefinition {
    StoryDefinition {
        id,
        summary,
        story_type: StoryType::Explorer,
        default_size: (100, 24),
        setup,
        build,
    }
}

fn empty() -> Result<StoryFixture> {
    Ok(StoryFixture::Explorer(ExplorerFixture::new().build()))
}

fn canonical() -> Result<StoryFixture> {
    let files = ExplorerFixture::new()
        .changes(|group| {
            group
                .modified("src/app.rs", 8, 3)
                .modified("src/components/button.rs", 4, 1)
                .modified("tests/app_test.rs", 12, 0)
                .modified("README.md", 2, 2);
        })
        .build();
    Ok(StoryFixture::Explorer(files))
}

fn long_list() -> Result<StoryFixture> {
    let files = ExplorerFixture::new()
        .changes(|group| {
            for number in 1..=40 {
                group.modified(&format!("story-{number:02}.rs"), number, number / 2);
            }
        })
        .build();
    Ok(StoryFixture::Explorer(files))
}

fn mixed_status() -> Result<StoryFixture> {
    let files = ExplorerFixture::new()
        .changes(|group| {
            group
                .modified("src/app.rs", 18, 7)
                .added("new feature.rs", 42)
                .deleted("docs/obsolete.md", 19)
                .untracked("notes/untracked draft.txt")
                .renamed("src/old_name.rs", "src/new_name.rs", 3, 2)
                .conflicted("src/conflict.rs");
        })
        .staged(|group| {
            group
                .modified("Cargo.toml", 1, 1)
                .added("migrations/20260904.sql", 120);
        })
        .build();
    Ok(StoryFixture::Explorer(files))
}

fn awkward_paths() -> Result<StoryFixture> {
    let files = ExplorerFixture::new()
        .changes(|group| {
            group
                .modified("src/中文.rs", 3, 1)
                .modified("docs/with spaces.rs", 7, 2)
                .modified("alpha/same-name.rs", 1, 1)
                .modified("beta/same-name.rs", 2, 2)
                .modified("one/two/three/four/five/deeply_nested.rs", 9, 4)
                .modified(
                    "a-very-long-directory-name/another-long-directory-name/a-very-long-file-name.rs",
                    15,
                    6,
                );
        })
        .build();
    Ok(StoryFixture::Explorer(files))
}

#[cfg(test)]
mod tests {
    use std::collections::HashSet;

    use file_types::ChangeType;

    use super::*;

    #[test]
    fn mixed_status_contains_every_change_type_and_both_groups() {
        let StoryFixture::Explorer(files) = mixed_status().unwrap() else {
            unreachable!()
        };
        let changes: Vec<ChangeType> = files
            .iter()
            .map(file_types::File::get_change_type)
            .collect();
        let headings: HashSet<&str> = files.iter().map(|file| file.revs().heading()).collect();

        for expected in [
            ChangeType::Modified,
            ChangeType::Added,
            ChangeType::Deleted,
            ChangeType::Untracked,
            ChangeType::Moved,
            ChangeType::Conflicted,
        ] {
            assert!(changes.contains(&expected), "missing {expected:?}");
        }
        assert_eq!(headings, HashSet::from(["Changes", "Staged Changes"]));
    }
}
