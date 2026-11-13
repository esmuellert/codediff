//! The contract between a story, its fixture, and either host.

use std::rc::Rc;

use anyhow::{Result, bail};
use diff_service::diff::DiffContent;
use file_types::File;
use loom::crokey::KeyCombination;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum StoryComponent {
    Welcome,
    Explorer,
    ExplorerCards,
    SideBySide,
    Inline,
    SingleFile,
    Card,
    Cards,
}

pub struct StoryDefinition {
    pub id: &'static str,
    pub description: &'static str,
    pub component: StoryComponent,
    pub compact: bool,
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
    ExplorerCards {
        files: Vec<File>,
        diffs: Vec<DiffContent>,
    },
    SideBySide(Rc<DiffContent>),
    Inline(Rc<DiffContent>),
    SingleFile(Rc<DiffContent>),
    Card(Rc<DiffContent>),
    Cards(Rc<Vec<Rc<DiffContent>>>),
}

impl StoryFixture {
    pub const fn component(&self) -> StoryComponent {
        match self {
            Self::Welcome => StoryComponent::Welcome,
            Self::Explorer(_) => StoryComponent::Explorer,
            Self::ExplorerCards { .. } => StoryComponent::ExplorerCards,
            Self::SideBySide(_) => StoryComponent::SideBySide,
            Self::Inline(_) => StoryComponent::Inline,
            Self::SingleFile(_) => StoryComponent::SingleFile,
            Self::Card(_) => StoryComponent::Card,
            Self::Cards(_) => StoryComponent::Cards,
        }
    }
}
