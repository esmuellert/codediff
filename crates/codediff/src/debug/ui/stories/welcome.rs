use anyhow::Result;

use super::super::definition::{StoryComponent, StoryDefinition, StoryFixture};

pub const STORIES: &[StoryDefinition] = &[StoryDefinition {
    id: "welcome/default",
    description: "DiffViewer with no selected file",
    component: StoryComponent::Welcome,
    snapshot_size: (100, 24),
    initial_keys: &[],
    make_fixture: welcome_fixture,
}];

fn welcome_fixture() -> Result<StoryFixture> {
    Ok(StoryFixture::Welcome)
}
