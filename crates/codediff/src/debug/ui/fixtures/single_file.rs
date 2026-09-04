//! One-sided file fixtures.

use std::rc::Rc;
use std::sync::Arc;

use file_types::File;
use pipeline::diff::{DiffContent, SingleFile};

use super::{at, worktree_revs};

#[derive(Clone, Copy)]
pub enum Presence {
    Added,
    Deleted,
}

pub struct SingleFileFixture {
    path: String,
    presence: Presence,
    lines: Vec<String>,
}

impl SingleFileFixture {
    pub fn from_lines(path: &str, presence: Presence, lines: &[&str]) -> Self {
        Self {
            path: path.to_owned(),
            presence,
            lines: lines.iter().map(|line| (*line).to_owned()).collect(),
        }
    }

    pub fn empty(path: &str, presence: Presence) -> Self {
        Self {
            path: path.to_owned(),
            presence,
            lines: Vec::new(),
        }
    }

    pub fn generated_rust(path: &str, lines: u32) -> Self {
        Self {
            path: path.to_owned(),
            presence: Presence::Added,
            lines: (1..=lines)
                .map(|number| format!("fn generated_{number:03}() -> usize {{ {number} }}"))
                .collect(),
        }
    }

    pub fn build(self) -> Rc<DiffContent> {
        let path = at(&self.path);
        let revs = worktree_revs();
        let file = match self.presence {
            Presence::Added => File::added(path, revs),
            Presence::Deleted => File::deleted(path, revs),
        };
        Rc::new(DiffContent::SingleFile(SingleFile {
            file,
            lines: Arc::new(self.lines),
        }))
    }
}
