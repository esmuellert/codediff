//! Version-control operations requested by components.

use file_types::{RepoPath, Rev, Revs};

pub struct VersionControlService;

impl VersionControlService {
    pub fn new() -> Self {
        Self
    }

    pub fn toggle_stage(&self, path: &RepoPath, revs: &Revs) {
        if !matches!(revs.after, Rev::Index | Rev::Worktree) {
            return;
        }
        let path = path.clone();
        let after = revs.after.clone();
        std::thread::spawn(move || {
            let Ok(repository) = vcs::Repository::open(path.root()) else {
                return;
            };
            let _ = match after {
                Rev::Index => repository.unstage(path.as_str()),
                Rev::Worktree => repository.stage(path.as_str()),
                Rev::Conflict(_) | Rev::Commit(_) => return,
            };
        });
    }
}

impl Default for VersionControlService {
    fn default() -> Self {
        Self::new()
    }
}
