use std::path::PathBuf;

use file_types::Rev;
use vcs::{DiffType, Repository};

struct Fixture {
    dir: PathBuf,
}

impl Fixture {
    fn new() -> Self {
        let dir = std::env::temp_dir().join(format!(
            "codediff-vcs-directory-stage-{}",
            std::process::id()
        ));
        fixtures::repo(&dir).expect("building the fixture repository");
        Self { dir }
    }

    fn files(&self) -> Vec<file_types::File> {
        Repository::open(&self.dir)
            .expect("opening the fixture repository")
            .get_changed_files(&DiffType::Worktree, &[])
            .expect("listing changed files")
    }
}

impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.dir);
    }
}

#[test]
fn a_directory_path_stages_and_unstages_its_files() {
    let fixture = Fixture::new();
    let repository = Repository::open(&fixture.dir).expect("opening the fixture repository");

    repository.stage("nest/b").expect("staging a directory");
    let files = fixture.files();
    for path in ["nest/b/two.txt", "nest/b/three.txt"] {
        let file = files
            .iter()
            .find(|file| file.path().as_str() == path)
            .expect("the staged file remains listed");
        assert_eq!(file.revs().after, Rev::Index);
    }

    repository.unstage("nest/b").expect("unstaging a directory");
    let files = fixture.files();
    for path in ["nest/b/two.txt", "nest/b/three.txt"] {
        let file = files
            .iter()
            .find(|file| file.path().as_str() == path)
            .expect("the unstaged file remains listed");
        assert_eq!(file.revs().after, Rev::Worktree);
    }
}
