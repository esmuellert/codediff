//! Typed builders for data injected into production components.

pub mod diff;
pub mod explorer;
pub mod single_file;

use std::path::Path;

use file_types::{Oid, RepoPath, Revs};

const STORY_ROOT: &str = "/codediff-story";
pub const MIN_LONG_LINE_CELLS: u32 = 512;

pub(super) fn repo_path(path: &str) -> RepoPath {
    RepoPath::new(path, Path::new(STORY_ROOT))
}

pub(super) fn worktree_revs() -> Revs {
    Revs::worktree_against(Oid::new("story-base"))
}

pub fn long_rust_line(name: &str, pattern: &str) -> String {
    assert!(!pattern.is_empty(), "a long-line pattern cannot be empty");
    let mut value = String::new();
    loop {
        let line = format!("pub const {name}: &str = \"{value}\";");
        let cells = line_index::LineIndex::new(&line, line_index::DEFAULT_TAB_WIDTH)
            .width()
            .get();
        if cells >= MIN_LONG_LINE_CELLS {
            return line;
        }
        value.push_str(pattern);
    }
}
