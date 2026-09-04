//! Typed builders for data injected into production components.

pub mod diff;
pub mod explorer;
pub mod single_file;

use std::path::Path;

use file_types::{Oid, RepoPath, Revs};

const ROOT: &str = "/codediff-story";
pub const LONG_LINE_MIN_CELLS: u32 = 512;

pub(super) fn at(path: &str) -> RepoPath {
    RepoPath::new(path, Path::new(ROOT))
}

pub(super) fn worktree_revs() -> Revs {
    Revs::worktree_against(Oid::new("story-base"))
}

pub fn long_rust_constant(name: &str, seed: &str) -> String {
    assert!(!seed.is_empty(), "a long-line seed cannot be empty");
    let mut value = String::new();
    loop {
        let line = format!("pub const {name}: &str = \"{value}\";");
        let cells = line_index::LineIndex::new(&line, line_index::DEFAULT_TAB_WIDTH)
            .width()
            .get();
        if cells >= LONG_LINE_MIN_CELLS {
            return line;
        }
        value.push_str(seed);
    }
}
