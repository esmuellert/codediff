//! Diff fixtures that still pass through the production engine and alignment.

use std::rc::Rc;

use anyhow::Result;
use file_types::File;
use pipeline::diff::{Diff, DiffContent};

use super::{repo_path, worktree_revs};

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

    pub fn compact(path: &str) -> Self {
        let mut original = vec![
            "fn compact_demo() {".to_owned(),
            "    let before_one = 1;".to_owned(),
            "    let before_two = 2;".to_owned(),
            "    let before_three = 3;".to_owned(),
            "    let first = \"old\";".to_owned(),
        ];
        for line in 6..=400 {
            original.push(format!("    let unchanged_{line:03} = {line};"));
        }
        original.push("}".to_owned());

        let mut modified = original.clone();
        modified[379] = "    let third = \"new\";".to_owned();
        modified.remove(380);
        modified.insert(380, "    let third_added = true;".to_owned());
        modified.remove(299);
        modified[219] = "    let second = \"new\";".to_owned();
        modified[220] = "    let second_context = 220;".to_owned();
        modified.insert(221, "    let second_added = true;".to_owned());
        modified.insert(99, "    let inserted_only = true;".to_owned());
        modified[14] = "    let second = \"old\";".to_owned();
        modified[15] = "    let second = \"new\";".to_owned();
        modified.insert(16, "    let second_added = true;".to_owned());
        modified[4] = "    let first = \"new\";".to_owned();
        modified.insert(5, "    let first_added = true;".to_owned());
        modified[6] = "    let first_context_changed = 6;".to_owned();

        Self {
            path: path.to_owned(),
            original,
            modified,
        }
    }

    pub fn with_line_pair(
        mut self,
        original: impl Into<String>,
        modified: impl Into<String>,
    ) -> Self {
        self.original.push(original.into());
        self.modified.push(modified.into());
        self
    }

    pub fn with_unchanged_lines(mut self, prefix: &str, count: u32) -> Self {
        for number in 1..=count {
            let line = format!("{prefix} {number:03}");
            self.original.push(line.clone());
            self.modified.push(line);
        }
        self
    }

    pub fn build(self) -> Result<Rc<DiffContent>> {
        let original: Vec<&str> = self.original.iter().map(String::as_str).collect();
        let modified: Vec<&str> = self.modified.iter().map(String::as_str).collect();
        let changed =
            pipeline::diff::compute(&original, &modified, pipeline::diff::Settings::default())?;
        let alignment = pipeline::diff::align(changed, &original, &modified)?;
        Ok(Rc::new(DiffContent::Diff(Diff {
            file: File::unchanged_path(repo_path(&self.path), worktree_revs()),
            alignment,
        })))
    }
}
