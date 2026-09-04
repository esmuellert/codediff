//! Explicit registration and stable ordering of every story.

use anyhow::Result;

use super::definition::StoryDefinition;
use super::stories::{explorer, side_by_side, single_file, welcome};

pub struct StoryGroup {
    pub label: &'static str,
    pub stories: &'static [StoryDefinition],
}

pub const GROUPS: &[StoryGroup] = &[
    StoryGroup {
        label: "Welcome",
        stories: welcome::STORIES,
    },
    StoryGroup {
        label: "Explorer",
        stories: explorer::STORIES,
    },
    StoryGroup {
        label: "Side by side",
        stories: side_by_side::STORIES,
    },
    StoryGroup {
        label: "Single file",
        stories: single_file::STORIES,
    },
];

pub fn stories() -> impl Iterator<Item = &'static StoryDefinition> {
    GROUPS.iter().flat_map(|group| group.stories.iter())
}

pub fn ids() -> impl Iterator<Item = &'static str> {
    stories().map(|story| story.id)
}

pub fn by_id(id: &str) -> Result<&'static StoryDefinition> {
    stories()
        .find(|story| story.id == id)
        .ok_or_else(|| anyhow::anyhow!("unknown UI story {id:?}; use --list to see story IDs"))
}

pub fn by_index(index: usize) -> Option<&'static StoryDefinition> {
    stories().nth(index)
}

pub fn story_count() -> usize {
    stories().count()
}

#[cfg(test)]
mod tests {
    use std::collections::HashSet;

    use super::*;

    #[test]
    fn story_ids_are_unique() {
        let story_ids: Vec<&str> = ids().collect();
        let unique: HashSet<&str> = story_ids.iter().copied().collect();
        assert_eq!(
            story_ids.len(),
            unique.len(),
            "duplicate story ID in catalog"
        );
    }
}
