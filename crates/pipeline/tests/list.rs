//! The file list a real repository produces, with the numbers the rows draw.

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
fn a_file_staged_and_edited_again_carries_a_count_per_comparison() {
    // What each explorer row draws. `staged-then-edited.txt` gained a line in
    // the working tree and swapped one in the index; both rows used to show
    // the staged pair.
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
