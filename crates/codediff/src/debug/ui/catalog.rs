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
        label: "SideBySide",
        stories: side_by_side::STORIES,
    },
    StoryGroup {
        label: "SingleFile",
        stories: single_file::STORIES,
    },
];

pub fn all() -> impl Iterator<Item = &'static StoryDefinition> {
    GROUPS.iter().flat_map(|group| group.stories.iter())
}

pub fn names() -> impl Iterator<Item = &'static str> {
    all().map(|story| story.id)
}

pub fn named(name: &str) -> Result<&'static StoryDefinition> {
    all()
        .find(|story| story.id == name)
        .ok_or_else(|| anyhow::anyhow!("unknown UI story {name:?}; use --list to see story names"))
}

pub fn at(index: usize) -> Option<&'static StoryDefinition> {
    all().nth(index)
}

pub fn len() -> usize {
    all().count()
}

#[cfg(test)]
mod tests {
    use std::collections::HashSet;

    use super::*;

    #[test]
    fn story_ids_are_unique() {
        let ids: Vec<&str> = names().collect();
        let unique: HashSet<&str> = ids.iter().copied().collect();
        assert_eq!(ids.len(), unique.len(), "duplicate story ID in catalog");
    }
}
