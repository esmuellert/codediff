//! Version-control operations requested by components.

use file_types::{RepoPath, Revs};

pub struct VersionControlService;

impl VersionControlService {
    pub fn new() -> Self {
        Self
    }

    pub fn toggle_stage(&self, path: &RepoPath, revs: &Revs) {
        let path = path.clone();
        let revs = revs.clone();
        std::thread::spawn(move || {
            let Ok(repository) = vcs::Repository::open(path.root()) else {
                return;
            };
            let _ = if revs.after == file_types::Rev::Index {
                repository.unstage(path.as_str())
            } else {
                repository.stage(path.as_str())
            };
        });
    }
}

impl Default for VersionControlService {
    fn default() -> Self {
        Self::new()
    }
}
