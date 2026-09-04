//! One-sided file fixtures.

use std::rc::Rc;
use std::sync::Arc;

use file_types::File;
use pipeline::diff::{DiffContent, SingleFile};

use super::{repo_path, worktree_revs};

#[derive(Clone, Copy)]
pub enum FilePresence {
    Added,
    Deleted,
}

pub struct SingleFileFixture {
    path: String,
    presence: FilePresence,
    lines: Vec<String>,
}

impl SingleFileFixture {
    pub fn from_lines(path: &str, presence: FilePresence, lines: &[&str]) -> Self {
        Self {
            path: path.to_owned(),
            presence,
            lines: lines.iter().map(|line| (*line).to_owned()).collect(),
        }
    }

    pub fn empty(path: &str, presence: FilePresence) -> Self {
        Self {
            path: path.to_owned(),
            presence,
            lines: Vec::new(),
        }
    }

    pub fn generated_rust(path: &str, line_count: u32) -> Self {
        Self {
            path: path.to_owned(),
            presence: FilePresence::Added,
            lines: (1..=line_count)
                .map(|number| format!("fn generated_{number:03}() -> usize {{ {number} }}"))
                .collect(),
        }
    }

    pub fn build(self) -> Rc<DiffContent> {
        let path = repo_path(&self.path);
        let revs = worktree_revs();
        let file = match self.presence {
            FilePresence::Added => File::added(path, revs),
            FilePresence::Deleted => File::deleted(path, revs),
        };
        Rc::new(DiffContent::SingleFile(SingleFile {
            file,
            lines: Arc::new(self.lines),
        }))
    }
}
