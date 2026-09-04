//! Diff fixtures that still pass through the production engine and alignment.

use std::rc::Rc;

use anyhow::Result;
use file_types::File;
use pipeline::diff::{Diff, DiffContent};

use super::{at, worktree_revs};

pub struct DiffFixture {
    path: String,
    original: Vec<String>,
    modified: Vec<String>,
}

impl DiffFixture {
    pub fn from_lines(path: &str, original: &[&str], modified: &[&str]) -> Self {
        Self {
            path: path.to_owned(),
            original: original.iter().map(|line| (*line).to_owned()).collect(),
            modified: modified.iter().map(|line| (*line).to_owned()).collect(),
        }
    }

    pub fn from_text(path: &str, original: &str, modified: &str) -> Self {
        Self {
            path: path.to_owned(),
            original: vscode_diff::editor_lines(original)
                .into_iter()
                .map(str::to_owned)
                .collect(),
            modified: vscode_diff::editor_lines(modified)
                .into_iter()
                .map(str::to_owned)
                .collect(),
        }
    }

    pub fn changed(mut self, original: impl Into<String>, modified: impl Into<String>) -> Self {
        self.original.push(original.into());
        self.modified.push(modified.into());
        self
    }

    pub fn pad_unchanged(mut self, prefix: &str, through: u32) -> Self {
        for number in 1..=through {
            let line = format!("{prefix} {number:03}");
            self.original.push(line.clone());
            self.modified.push(line);
        }
        self
    }

    pub fn build(self) -> Result<Rc<DiffContent>> {
        let original: Vec<&str> = self.original.iter().map(String::as_str).collect();
        let modified: Vec<&str> = self.modified.iter().map(String::as_str).collect();
        let changed = pipeline::diff::compute(&original, &modified)?;
        let alignment = pipeline::diff::align(changed, &original, &modified)?;
        Ok(Rc::new(DiffContent::Diff(Diff {
            file: File::unchanged_path(at(&self.path), worktree_revs()),
            alignment,
        })))
    }
}
