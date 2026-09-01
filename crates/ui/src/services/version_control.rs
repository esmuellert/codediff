//! Version-control operations requested by components.

use file_types::File;

pub struct VersionControlService;

impl VersionControlService {
    pub fn new() -> Self {
        Self
    }

    pub fn toggle_stage(&self, file: &File) {
        let repository_root = file.path().root().to_path_buf();
        let relative_path = file.path().as_str().to_owned();
        let is_staged = file.revs().after == file_types::Rev::Index;
        std::thread::spawn(move || {
            let Ok(repository) = vcs::Repository::open(&repository_root) else {
                return;
            };
            let _ = if is_staged {
                repository.unstage(&relative_path)
            } else {
                repository.stage(&relative_path)
            };
        });
    }
}

impl Default for VersionControlService {
    fn default() -> Self {
        Self::new()
    }
}
