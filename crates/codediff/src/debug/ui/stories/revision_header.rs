use anyhow::Result;
use file_types::Oid;
use ui::Commit;
use ui::components::RevisionSide;

use super::super::definition::{StoryComponent, StoryDefinition, StoryFixture};

pub const STORIES: &[StoryDefinition] = &[
    StoryDefinition {
        id: "revision-header/default",
        description: "Single-row revision header with repo and branch",
        component: StoryComponent::RevisionHeader,
        compact: false,
        snapshot_size: (110, 6),
        initial_keys: &[],
        make_fixture: default_fixture,
    },
    StoryDefinition {
        id: "revision-header/picking",
        description: "Revision header with left endpoint active",
        component: StoryComponent::RevisionHeader,
        compact: false,
        snapshot_size: (110, 6),
        initial_keys: &[],
        make_fixture: picking_fixture,
    },
];

fn sample_commits() -> Vec<Commit> {
    vec![Commit {
        oid: Oid::new("6152138abcdef0123456789"),
        parents: Vec::new(),
        current: true,
        branches: vec!["main".to_owned()],
        remotes: Vec::new(),
        tags: Vec::new(),
        subject: "feat: add revision header".to_owned(),
        author: "Alex".to_owned(),
        date: "2 hours ago".to_owned(),
    }]
}

fn default_fixture() -> Result<StoryFixture> {
    Ok(StoryFixture::RevisionHeader {
        commits: sample_commits(),
        active: None,
    })
}

fn picking_fixture() -> Result<StoryFixture> {
    Ok(StoryFixture::RevisionHeader {
        commits: sample_commits(),
        active: Some(RevisionSide::Left),
    })
}
