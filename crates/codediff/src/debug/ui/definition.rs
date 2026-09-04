//! The contract between a story, its fixture, and either host.

use std::rc::Rc;

use anyhow::{Result, bail};
use file_types::{DiffVersion, File};
use loom::crokey::KeyCombination;
use pipeline::diff::DiffContent;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum StoryComponent {
    Welcome,
    Explorer,
    SideBySide,
    SingleFile,
}

pub struct StoryDefinition {
    pub id: &'static str,
    pub description: &'static str,
    pub component: StoryComponent,
    pub snapshot_size: (u16, u16),
    pub initial_keys: &'static [KeyCombination],
    pub make_fixture: fn() -> Result<StoryFixture>,
}

impl StoryDefinition {
    pub fn create_fixture(&self) -> Result<StoryFixture> {
        let fixture = (self.make_fixture)()?;
        if fixture.component() != self.component {
            bail!("story {} built the wrong component", self.id);
        }
        Ok(fixture)
    }
}

pub enum StoryFixture {
    Welcome,
    Explorer(Vec<File>),
    SideBySide(Rc<DiffContent>),
    SingleFile(Rc<DiffContent>),
}

impl StoryFixture {
    pub const fn component(&self) -> StoryComponent {
        match self {
            Self::Welcome => StoryComponent::Welcome,
            Self::Explorer(_) => StoryComponent::Explorer,
            Self::SideBySide(_) => StoryComponent::SideBySide,
            Self::SingleFile(_) => StoryComponent::SingleFile,
        }
    }

    pub const fn needs_syntax(&self) -> bool {
        matches!(self, Self::SideBySide(_) | Self::SingleFile(_))
    }

    pub fn initial_syntax_response_count(&self) -> usize {
        match self {
            Self::SideBySide(content) => {
                let DiffContent::Diff(diff) = content.as_ref() else {
                    unreachable!()
                };
                usize::from(!diff.alignment.lines(DiffVersion::Original).is_empty())
                    + usize::from(!diff.alignment.lines(DiffVersion::Modified).is_empty())
            }
            Self::SingleFile(content) => {
                let DiffContent::SingleFile(single) = content.as_ref() else {
                    unreachable!()
                };
                usize::from(!single.lines.is_empty())
            }
            Self::Welcome | Self::Explorer(_) => 0,
        }
    }
}
