use anyhow::Result;

use super::super::definition::{StoryDefinition, StoryFixture, StoryType};

pub const STORIES: &[StoryDefinition] = &[StoryDefinition {
    id: "welcome/default",
    summary: "DiffViewer with no selected file",
    story_type: StoryType::Welcome,
    default_size: (100, 24),
    setup: &[],
    build: default,
}];

fn default() -> Result<StoryFixture> {
    Ok(StoryFixture::Welcome)
}
