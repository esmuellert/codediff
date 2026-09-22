//! The file list a real repository produces, with the numbers the lines draw.

use std::path::PathBuf;

use file_types::Stats;
use pipeline::files::{self as files, Request};

/// A fixture repository in a temporary directory, removed on drop.
struct Fixture {
    dir: PathBuf,
}

impl Fixture {
    fn new(name: &str) -> Self {
        let dir =
            std::env::temp_dir().join(format!("codediff-pipeline-{name}-{}", std::process::id()));
        fixtures::repo(&dir).expect("building the fixture repository");
        Self { dir }
    }
}

impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.dir);
    }
}

#[test]
fn untracked_and_deleted_files_carry_one_sided_counts() {
    let fixture = Fixture::new("one-sided-counts");
    let files = files::get_files(&Request::worktree(&fixture.dir)).expect("listing");

    let untracked = files
        .iter()
        .find(|file| file.path().as_str() == "untracked.txt")
        .expect("untracked file is listed");
    assert_eq!(untracked.get_stats(), Some(Stats::new(1, 0)));

    let deleted = files
        .iter()
        .find(|file| file.path().as_str() == "deleted.txt")
        .expect("deleted file is listed");
    assert_eq!(deleted.get_stats(), Some(Stats::new(0, 1)));
}

#[test]
fn a_file_staged_and_edited_again_carries_a_count_per_comparison() {
    // Each comparison has its own line counts.
    let fixture = Fixture::new("counts");
    let files = files::get_files(&Request::worktree(&fixture.dir)).expect("listing");

    let found: Vec<(&'static str, Option<Stats>)> = files
        .iter()
        .filter(|file| file.path().as_str() == "staged-then-edited.txt")
        .map(|file| (file.revs().heading(), file.get_stats()))
        .collect();
    assert_eq!(
        found,
        vec![
            ("Changes", Some(Stats::new(1, 0))),
            ("Staged Changes", Some(Stats::new(1, 1))),
        ],
        "{found:?}"
    );
}
