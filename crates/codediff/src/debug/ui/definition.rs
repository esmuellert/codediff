//! The contract between a story, its fixture, and either host.

use std::path::PathBuf;
use std::rc::Rc;

use anyhow::{Result, bail};
use diff_service::diff::DiffContent;
use file_types::File;
use loom::crokey::KeyCombination;
use vcs::{Commit, RevisionRange};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum StoryComponent {
    Welcome,
    Explorer,
    ExplorerCards,
    CommitGraph,
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

/// A temporary Git repository removed when the story closes.
pub struct LiveRepo {
    dir: PathBuf,
}

impl LiveRepo {
    pub fn new(dir: PathBuf) -> Self {
        Self { dir }
    }

    pub fn path(&self) -> &std::path::Path {
        &self.dir
    }
}

impl Drop for LiveRepo {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.dir);
    }
}

pub enum StoryFixture {
    Welcome,
    Explorer(Vec<File>),
    ExplorerCards {
        files: Vec<File>,
        diffs: Vec<DiffContent>,
    },
    Repository {
        repo: LiveRepo,
        revision_range: RevisionRange,
    },
    CommitGraph {
        commits: Vec<Commit>,
        branch_drawing: bool,
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
            Self::ExplorerCards { .. } | Self::Repository { .. } => StoryComponent::ExplorerCards,
            Self::CommitGraph { .. } => StoryComponent::CommitGraph,
            Self::SideBySide(_) => StoryComponent::SideBySide,
            Self::Inline(_) => StoryComponent::Inline,
            Self::SingleFile(_) => StoryComponent::SingleFile,
            Self::Card(_) => StoryComponent::Card,
            Self::Cards(_) => StoryComponent::Cards,
        }
    }
}
