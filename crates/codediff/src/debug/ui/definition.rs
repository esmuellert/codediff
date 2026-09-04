//! The contract between a story, its fixture, and either host.

use std::rc::Rc;

use anyhow::{Result, bail};
use file_types::{DiffVersion, File};
use loom::crokey::KeyCombination;
use pipeline::diff::DiffContent;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum StoryType {
    Welcome,
    Explorer,
    SideBySide,
    SingleFile,
}

pub struct StoryDefinition {
    pub id: &'static str,
    pub summary: &'static str,
    pub story_type: StoryType,
    pub default_size: (u16, u16),
    pub setup: &'static [KeyCombination],
    pub build: fn() -> Result<StoryFixture>,
}

impl StoryDefinition {
    pub fn build(&self) -> Result<StoryFixture> {
        let fixture = (self.build)()?;
        if fixture.story_type() != self.story_type {
            bail!("story {} built the wrong component type", self.id);
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
    pub const fn story_type(&self) -> StoryType {
        match self {
            Self::Welcome => StoryType::Welcome,
            Self::Explorer(_) => StoryType::Explorer,
            Self::SideBySide(_) => StoryType::SideBySide,
            Self::SingleFile(_) => StoryType::SingleFile,
        }
    }

    pub const fn needs_syntax(&self) -> bool {
        matches!(self, Self::SideBySide(_) | Self::SingleFile(_))
    }

    pub fn syntax_responses(&self) -> usize {
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
